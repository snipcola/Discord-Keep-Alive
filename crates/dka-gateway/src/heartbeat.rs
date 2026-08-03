use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, interval, sleep};
use tracing::trace;

use crate::reconnect::unit_f64;

pub enum HeartbeatCmd {
  Send { seq: Value },
  Zombie,
}

pub async fn heartbeat_loop(
  interval_ms: u64,
  tx: mpsc::UnboundedSender<HeartbeatCmd>,
  mut ack_rx: watch::Receiver<Instant>,
  seq_cell: Arc<AtomicI64>,
  shutdown: &mut watch::Receiver<bool>,
) {
  // First heartbeat after interval * random(0..1); then every interval.
  if *shutdown.borrow() {
    return;
  }
  let jitter_ms = (interval_ms as f64 * unit_f64()) as u64;
  tokio::select! {
    _ = sleep(Duration::from_millis(jitter_ms)) => {}
    changed = shutdown.changed() => {
      if changed.is_err() || *shutdown.borrow() {
        return;
      }
    }
  }

  // First tick is immediate; intentional after the jitter wait above.
  let mut ticker = interval(Duration::from_millis(interval_ms));
  ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

  let mut waiting_ack = false;

  loop {
    tokio::select! {
      changed = shutdown.changed() => {
        if changed.is_err() || *shutdown.borrow() {
          return;
        }
      }
      result = ack_rx.changed() => {
        if result.is_err() {
          return;
        }
        let _ = ack_rx.borrow_and_update();
        waiting_ack = false;
      }
      _ = ticker.tick() => {
        if waiting_ack {
          let _ = tx.send(HeartbeatCmd::Zombie);
          return;
        }
        let raw = seq_cell.load(Ordering::Relaxed);
        let seq = if raw < 0 {
          Value::Null
        } else {
          json!(raw)
        };
        if tx.send(HeartbeatCmd::Send { seq }).is_err() {
          return;
        }
        waiting_ack = true;
        trace!("heartbeat scheduled");
      }
    }
  }
}
