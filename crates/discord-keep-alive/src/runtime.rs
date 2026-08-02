use std::sync::Arc;

use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{Instrument, error, info, info_span};

use crate::config::AccountConfig;
use crate::gateway::properties::Defaults;
use crate::gateway::run_session;
use crate::health::HealthState;

pub async fn run(
  accounts: Vec<AccountConfig>,
  defaults: Defaults,
  health: Option<Arc<HealthState>>,
  shutdown: watch::Receiver<bool>,
) {
  let mut set = JoinSet::new();

  for account in accounts {
    let rx = shutdown.clone();
    let defaults = defaults.clone();
    let health = health.clone();
    info!(account = %account.name, "starting session");
    set.spawn(async move {
      run_account(account, defaults, health, rx).await;
    });
  }

  while let Some(res) = set.join_next().await {
    if let Err(err) = res {
      error!(error = %err, "account task failed");
    }
  }
}

async fn run_account(
  account: AccountConfig,
  defaults: Defaults,
  health: Option<Arc<HealthState>>,
  shutdown: watch::Receiver<bool>,
) {
  let name = account.name.clone();
  let span = info_span!("session", account = %name);

  async move {
    if let Err(err) = run_session(account, defaults, health, shutdown).await {
      error!(error = %err, "session failed");
    }
  }
  .instrument(span)
  .await;
}
