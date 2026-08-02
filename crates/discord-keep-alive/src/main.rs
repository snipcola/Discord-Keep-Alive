mod config;
mod gateway;
mod log;
mod runtime;

use std::time::Duration;

use clap::Parser;
use tokio::sync::watch;
use tracing::{error, info};

use crate::config::{Cli, load};

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::main]
async fn main() {
  rustls::crypto::ring::default_provider()
    .install_default()
    .expect("install rustls CryptoProvider");

  let cli = Cli::parse();

  let app = match load(&cli) {
    Ok(cfg) => cfg,
    Err(err) => {
      eprintln!("error: {err}");
      std::process::exit(1);
    }
  };

  log::init(&app.log_level);

  info!(accounts = app.accounts.len(), "starting");

  let (shutdown_tx, shutdown_rx) = watch::channel(false);

  let supervisor = tokio::spawn(runtime::run(app.accounts, app.defaults, shutdown_rx));

  wait_for_shutdown_signal().await;
  info!("shutting down");
  let _ = shutdown_tx.send(true);

  match tokio::time::timeout(SHUTDOWN_TIMEOUT, supervisor).await {
    Ok(Ok(())) => {}
    Ok(Err(err)) => {
      error!(error = %err, "supervisor failed");
      std::process::exit(1);
    }
    Err(_) => {
      error!(
        timeout_secs = SHUTDOWN_TIMEOUT.as_secs(),
        "shutdown timed out"
      );
      std::process::exit(1);
    }
  }
}

async fn wait_for_shutdown_signal() {
  #[cfg(unix)]
  {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigterm = signal(SignalKind::terminate()).expect("register SIGTERM");
    tokio::select! {
      _ = tokio::signal::ctrl_c() => {}
      _ = sigterm.recv() => {}
    }
  }

  #[cfg(not(unix))]
  {
    let _ = tokio::signal::ctrl_c().await;
  }
}
