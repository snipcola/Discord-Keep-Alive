mod config;
mod gateway;
mod health;
mod log;
mod runtime;

use std::time::Duration;

use clap::Parser;
use tokio::sync::watch;
use tracing::{error, info};

use crate::config::{Cli, Command, load, load_health_endpoint};
use crate::health::HealthState;

const SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(15);

#[tokio::main]
async fn main() {
  let cli = Cli::parse();

  if let Some(Command::Health {
    health_socket,
    config,
  }) = cli.command
  {
    let code = run_health_probe(health_socket.as_deref(), &config).await;
    std::process::exit(code);
  }

  rustls::crypto::ring::default_provider()
    .install_default()
    .expect("install rustls CryptoProvider");

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

  let health = app
    .health_socket
    .as_ref()
    .map(|_| HealthState::new(app.accounts.iter().map(|a| a.name.clone())));

  let mut health_task = None;
  if let (Some(endpoint), Some(state)) = (app.health_socket.clone(), health.clone()) {
    let rx = shutdown_rx.clone();
    health_task = Some(tokio::spawn(async move {
      if let Err(err) = health::serve(&endpoint, state, rx).await {
        error!(error = %err, "health server failed");
      }
    }));
  }

  let supervisor = tokio::spawn(runtime::run(
    app.accounts,
    app.defaults,
    health,
    shutdown_rx,
  ));

  wait_for_shutdown_signal().await;
  info!("shutting down");
  let _ = shutdown_tx.send(true);

  if let Some(task) = health_task {
    let _ = task.await;
  }

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

async fn run_health_probe(cli_override: Option<&str>, config_path: &std::path::Path) -> i32 {
  let endpoint = match load_health_endpoint(config_path, cli_override) {
    Ok(Some(endpoint)) => endpoint,
    Ok(None) => {
      eprintln!(
        "error: health socket not configured (set health_socket / HEALTH_SOCKET / --health-socket)"
      );
      return 2;
    }
    Err(err) => {
      eprintln!("error: {err}");
      return 2;
    }
  };

  health::probe(&endpoint).await
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
