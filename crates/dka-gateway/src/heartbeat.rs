use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use tokio::sync::{mpsc, watch};
use tokio::time::{Instant, MissedTickBehavior, interval, sleep};
use tracing::trace;

use crate::is_shutdown;

pub enum HeartbeatCmd {
  Send { seq: Option<i64> },
  Zombie { interval_ms: u64, since_ack_ms: u64 },
}

pub async fn heartbeat_loop(
  interval_ms: u64,
  tx: mpsc::UnboundedSender<HeartbeatCmd>,
  mut ack_rx: watch::Receiver<Instant>,
  seq_cell: Arc<AtomicI64>,
  shutdown: &mut watch::Receiver<bool>,
) {
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

  // interval() ticks immediately, which is fine after the jitter sleep above.
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
        let seq = (raw >= 0).then_some(raw);
        if tx.send(HeartbeatCmd::Send { seq }).is_err() {
          return;
        }
        waiting_ack = true;
        trace!("heartbeat scheduled");
      }
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  const INTERVAL_MS: u64 = 1000;

  struct Harness {
    rx: mpsc::UnboundedReceiver<HeartbeatCmd>,
    ack_tx: watch::Sender<Instant>,
    _shutdown_tx: watch::Sender<bool>,
  }

  fn start(initial_seq: i64) -> Harness {
    let (tx, rx) = mpsc::unbounded_channel();
    let (ack_tx, ack_rx) = watch::channel(Instant::now());
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let seq_cell = Arc::new(AtomicI64::new(initial_seq));
    tokio::spawn(async move {
      heartbeat_loop(INTERVAL_MS, tx, ack_rx, seq_cell, &mut shutdown_rx).await;
    });
    Harness {
      rx,
      ack_tx,
      _shutdown_tx: shutdown_tx,
    }
  }

  async fn expect_send(rx: &mut mpsc::UnboundedReceiver<HeartbeatCmd>, case: &str) -> Option<i64> {
    match rx.recv().await {
      Some(HeartbeatCmd::Send { seq }) => seq,
      Some(HeartbeatCmd::Zombie { .. }) => panic!("{case}: expected a heartbeat, got zombie"),
      None => panic!("{case}: expected a heartbeat, but the loop stopped"),
    }
  }

  #[tokio::test(start_paused = true)]
  async fn missed_ack_reports_zombie_after_one_interval() {
    let mut hb = start(5);
    assert_eq!(
      expect_send(&mut hb.rx, "first beat").await,
      Some(5),
      "first beat carries the stored sequence"
    );
    match hb.rx.recv().await {
      Some(HeartbeatCmd::Zombie {
        interval_ms,
        since_ack_ms,
      }) => {
        assert_eq!(
          interval_ms, INTERVAL_MS,
          "zombie reports the hello interval"
        );
        assert!(
          since_ack_ms >= INTERVAL_MS,
          "zombie reported {since_ack_ms} ms since the last ack, expected at least one interval"
        );
      }
      Some(HeartbeatCmd::Send { .. }) => panic!("unacked beat must end in zombie, not a new beat"),
      None => panic!("loop stopped without reporting the missed ack"),
    }
    assert!(
      hb.rx.recv().await.is_none(),
      "loop must stop after reporting zombie"
    );
  }

  #[tokio::test(start_paused = true)]
  async fn acked_beats_keep_the_loop_beating() {
    let mut hb = start(-1);
    assert_eq!(
      expect_send(&mut hb.rx, "first beat").await,
      None,
      "negative stored sequence means no sequence yet"
    );
    hb.ack_tx.send(Instant::now()).unwrap();
    assert_eq!(
      expect_send(&mut hb.rx, "second beat after ack").await,
      None,
      "acked beat must be followed by another beat"
    );
    hb.ack_tx.send(Instant::now()).unwrap();
    assert_eq!(
      expect_send(&mut hb.rx, "third beat after ack").await,
      None,
      "loop must keep beating while acks arrive"
    );
  }

  #[tokio::test(start_paused = true)]
  async fn loop_stops_on_shutdown() {
    let (tx, mut rx) = mpsc::unbounded_channel::<HeartbeatCmd>();
    let (_ack_tx, ack_rx) = watch::channel(Instant::now());
    let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
    let task = tokio::spawn(async move {
      heartbeat_loop(
        INTERVAL_MS,
        tx,
        ack_rx,
        Arc::new(AtomicI64::new(-1)),
        &mut shutdown_rx,
      )
      .await;
    });
    expect_send(&mut rx, "first beat").await;
    shutdown_tx.send(true).unwrap();
    task
      .await
      .expect("heartbeat task must finish after shutdown");
    assert!(
      rx.recv().await.is_none(),
      "no further commands after shutdown"
    );
  }
}
