use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, interval, sleep};
use tracing::trace;

use crate::is_shutdown;

pub enum HeartbeatCmd {
  Send { seq: Value },
  Zombie { interval_ms: u64, since_ack_ms: u64 },
}

pub async fn heartbeat_loop(
  interval_ms: u64,
  tx: mpsc::UnboundedSender<HeartbeatCmd>,
  mut ack_rx: watch::Receiver<Instant>,
  seq_cell: Arc<AtomicI64>,
  shutdown: &mut watch::Receiver<bool>,
) {
  // First beat is delayed by interval * random(0..1); later beats are every interval.
  if is_shutdown(shutdown) {
    return;
  }
  let jitter_ms = (interval_ms as f64 * rand::random::<f64>()) as u64;
  tokio::select! {
    _ = sleep(Duration::from_millis(jitter_ms)) => {}
    changed = shutdown.changed() => {
      if changed.is_err() || is_shutdown(shutdown) {
        return;
      }
    }
  }

  // interval() fires immediately; that is fine after the jitter sleep above.
  let mut ticker = interval(Duration::from_millis(interval_ms));
  ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);

  let mut waiting_ack = false;

  loop {
    tokio::select! {
      changed = shutdown.changed() => {
        if changed.is_err() || is_shutdown(shutdown) {
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
          let since_ack_ms = ack_rx.borrow().elapsed().as_millis() as u64;
          let _ = tx.send(HeartbeatCmd::Zombie {
            interval_ms,
            since_ack_ms,
          });
          return;
        }
        let raw = seq_cell.load(Ordering::Relaxed);
        let seq = if raw < 0 { Value::Null } else { json!(raw) };
        if tx.send(HeartbeatCmd::Send { seq }).is_err() {
          return;
        }
        waiting_ack = true;
        trace!("heartbeat scheduled");
      }
    }
  }
}
