use std::time::Duration;

use super::gateway_query;

const MAX_BACKOFF_SECS: u64 = 90;

/// Exponential reconnect delay (1s, 2s, 4s, ..., cap 90s) plus up to 25% jitter.
/// `attempt` is 1 on the first reconnect.
pub fn backoff_with_jitter(attempt: u32) -> Duration {
  let exp = attempt.saturating_sub(1).min(7);
  let base_ms = (1u64 << exp).min(MAX_BACKOFF_SECS).saturating_mul(1000);
  let jitter_ms = (unit_f64() * base_ms as f64 * 0.25) as u64;
  Duration::from_millis(base_ms + jitter_ms)
}

/// Gateway WebSocket URL from READY's resume host.
pub fn resume_ws_url(base: &str) -> String {
  let query = gateway_query();
  if base.contains('?') {
    format!("{base}&{query}")
  } else {
    let trimmed = base.trim_end_matches('/');
    format!("{trimmed}/?{query}")
  }
}

/// Random float in `[0, 1)` via a small thread-local PRNG (no `rand` crate).
pub fn unit_f64() -> f64 {
  use std::cell::Cell;
  use std::collections::hash_map::DefaultHasher;
  use std::hash::{Hash, Hasher};
  use std::time::{SystemTime, UNIX_EPOCH};

  thread_local! {
    static STATE: Cell<u64> = const { Cell::new(0) };
  }

  STATE.with(|cell| {
    let mut s = cell.get();
    if s == 0 {
      let mut h = DefaultHasher::new();
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
        .hash(&mut h);
      std::thread::current().id().hash(&mut h);
      s = h.finish() | 1;
    }
    s ^= s << 13;
    s ^= s >> 7;
    s ^= s << 17;
    cell.set(s);
    (s as f64) / (u64::MAX as f64 + 1.0)
  })
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn backoff_escalates_then_caps() {
    let a1 = backoff_with_jitter(1).as_millis();
    let a4 = backoff_with_jitter(4).as_millis();
    let a20 = backoff_with_jitter(20).as_millis();
    assert!((1000..=1250).contains(&a1));
    assert!((8000..=10_000).contains(&a4));
    assert!((90_000..=112_500).contains(&a20));
  }

  #[test]
  fn early_backoff_retains_subsecond_jitter() {
    let mut saw_fractional = false;
    for _ in 0..64 {
      let ms = backoff_with_jitter(1).as_millis();
      assert!((1000..=1250).contains(&ms));
      if ms > 1000 {
        saw_fractional = true;
        break;
      }
    }
    assert!(
      saw_fractional,
      "expected at least one attempt-1 delay with non-zero sub-second jitter"
    );
  }

  #[test]
  fn resume_url_appends_query() {
    let query = crate::gateway::gateway_query();
    assert_eq!(
      resume_ws_url("wss://gateway-us-east1-b.discord.gg"),
      format!("wss://gateway-us-east1-b.discord.gg/?{query}")
    );
    assert_eq!(
      resume_ws_url("wss://gateway.discord.gg/?v=9"),
      format!("wss://gateway.discord.gg/?v=9&{query}")
    );
  }

  #[test]
  fn unit_f64_in_unit_interval() {
    for _ in 0..32 {
      let v = unit_f64();
      assert!((0.0..1.0).contains(&v));
    }
  }
}
