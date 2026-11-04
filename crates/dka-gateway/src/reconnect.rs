use std::time::Duration;

use crate::gateway_query;

const MAX_BACKOFF_SECS: u64 = 90;

pub fn next_attempt(current: u32, session_was_healthy: bool) -> u32 {
  if session_was_healthy {
    1
  } else {
    current.saturating_add(1)
  }
}

pub fn backoff_with_jitter(attempt: u32) -> Duration {
  backoff_delay(attempt, rand::random())
}

fn backoff_delay(attempt: u32, jitter_fraction: f64) -> Duration {
  let exp = attempt.saturating_sub(1).min(7);
  let base_ms = (1u64 << exp).min(MAX_BACKOFF_SECS).saturating_mul(1000);
  let jitter_ms = (jitter_fraction * base_ms as f64 * 0.25) as u64;
  Duration::from_millis(base_ms + jitter_ms)
}

pub fn resume_ws_url(base: &str) -> String {
  let query = gateway_query();
  if base.contains('?') {
    format!("{base}&{query}")
  } else {
    let trimmed = base.trim_end_matches('/');
    format!("{trimmed}/?{query}")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn attempts_count_failures_and_reset_after_a_healthy_session() {
    for (current, healthy, expected) in [
      (0, false, 1),
      (1, false, 2),
      (6, false, 7),
      (6, true, 1),
      (0, true, 1),
      (u32::MAX, false, u32::MAX),
    ] {
      assert_eq!(
        next_attempt(current, healthy),
        expected,
        "after attempt {current} with healthy session = {healthy}"
      );
    }
  }

  #[test]
  fn backoff_delay_table() {
    for (attempt, jitter, expected_ms) in [
      (0, 0.0, 1000),
      (1, 0.0, 1000),
      (1, 1.0, 1250),
      (2, 0.0, 2000),
      (3, 0.0, 4000),
      (4, 0.0, 8000),
      (4, 0.5, 9000),
      (7, 0.0, 64_000),
      (8, 0.0, 90_000),
      (8, 1.0, 112_500),
      (20, 0.0, 90_000),
      (u32::MAX, 0.0, 90_000),
    ] {
      assert_eq!(
        backoff_delay(attempt, jitter),
        Duration::from_millis(expected_ms),
        "attempt {attempt} with jitter fraction {jitter}"
      );
    }
  }

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
    let query = gateway_query();
    assert_eq!(
      resume_ws_url("wss://gateway-us-east1-b.discord.gg"),
      format!("wss://gateway-us-east1-b.discord.gg/?{query}")
    );
    assert_eq!(
      resume_ws_url("wss://gateway.discord.gg/?v=9"),
      format!("wss://gateway.discord.gg/?v=9&{query}")
    );
  }
}
