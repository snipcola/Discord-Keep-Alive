mod serve;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use thiserror::Error;
use tracing::debug;

pub use serve::serve;

const PROBE_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug)]
pub struct HealthState {
  live: Mutex<HashMap<String, bool>>,
}

impl HealthState {
  pub fn new(accounts: impl IntoIterator<Item = impl Into<String>>) -> Arc<Self> {
    let live = accounts
      .into_iter()
      .map(|name| (name.into(), false))
      .collect();
    Arc::new(Self {
      live: Mutex::new(live),
    })
  }

  pub fn set_live(&self, account: &str, live: bool) {
    let mut map = self.live.lock().unwrap_or_else(|e| e.into_inner());
    if let Some(slot) = map.get_mut(account)
      && *slot != live
    {
      *slot = live;
      debug!(account = %account, live, "health live state updated");
    }
  }

  pub fn status_line(&self) -> &'static str {
    let map = self.live.lock().unwrap_or_else(|e| e.into_inner());
    if !map.is_empty() && map.values().all(|&live| live) {
      "ok\n"
    } else {
      "fail\n"
    }
  }
}

#[derive(Debug, Error)]
pub enum ProbeError {
  #[error("unhealthy: one or more accounts are not live")]
  Unhealthy,
  #[error("health probe failed: {0:#}")]
  Unreachable(anyhow::Error),
  #[error("health probe timed out")]
  TimedOut,
}

impl ProbeError {
  pub fn exit_code(&self) -> i32 {
    match self {
      Self::Unhealthy => 1,
      Self::Unreachable(_) | Self::TimedOut => 2,
    }
  }
}

pub async fn probe(endpoint: &str) -> Result<(), ProbeError> {
  match tokio::time::timeout(PROBE_TIMEOUT, serve::probe_once(endpoint)).await {
    Ok(Ok(true)) => Ok(()),
    Ok(Ok(false)) => Err(ProbeError::Unhealthy),
    Ok(Err(err)) => Err(ProbeError::Unreachable(err)),
    Err(_) => Err(ProbeError::TimedOut),
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn healthy_when_all_live() {
    let state = HealthState::new(["a", "b"]);
    assert_eq!(state.status_line(), "fail\n");

    state.set_live("a", true);
    assert_eq!(state.status_line(), "fail\n");

    state.set_live("b", true);
    assert_eq!(state.status_line(), "ok\n");

    state.set_live("a", false);
    assert_eq!(state.status_line(), "fail\n");
  }

  #[test]
  fn empty_accounts_unhealthy() {
    let state = HealthState::new(std::iter::empty::<String>());
    assert_eq!(state.status_line(), "fail\n");
  }
}
