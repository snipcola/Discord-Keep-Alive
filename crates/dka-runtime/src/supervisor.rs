use std::sync::Arc;

use dka_gateway::{LiveSink, SessionParams, run_session};
use tokio::sync::watch;
use tokio::task::JoinSet;
use tracing::{Instrument, error, info, info_span};

use crate::health::HealthState;

pub async fn run_accounts(
  accounts: Vec<SessionParams>,
  health: Option<Arc<HealthState>>,
  shutdown: watch::Receiver<bool>,
) {
  let mut set = JoinSet::new();

  for params in accounts {
    let rx = shutdown.clone();
    let health = health.clone();
    info!(account = %params.name, "starting session");
    set.spawn(async move {
      run_account(params, health, rx).await;
    });
  }

  while let Some(res) = set.join_next().await {
    if let Err(err) = res {
      error!(error = %err, "account task failed");
    }
  }
}

async fn run_account(
  params: SessionParams,
  health: Option<Arc<HealthState>>,
  shutdown: watch::Receiver<bool>,
) {
  let name = params.name.clone();
  let span = info_span!("session", account = %name);

  async move {
    let account_name = params.name.clone();
    let on_live: LiveSink = match health {
      Some(state) => Box::new(move |live| state.set_live(&account_name, live)),
      None => Box::new(|_| {}),
    };
    if let Err(err) = run_session(params, on_live, shutdown).await {
      error!(error = %err, "session failed");
    }
  }
  .instrument(span)
  .await;
}
