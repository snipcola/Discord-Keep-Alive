mod adapt;
mod log;

use std::time::Duration;

use clap::Parser;
use dka_config::{Cli, Command, load, load_health_endpoint};
use dka_runtime::HealthState;
use tokio::sync::watch;
use tracing::{error, info};

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

  let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
  let (reason_tx, reason_rx) = watch::channel(None::<&'static str>);
  spawn_shutdown_listener(shutdown_tx, reason_tx);

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

  if *shutdown_rx.borrow() {
    log_shutting_down(&reason_rx);
    return;
  }

  info!(accounts = app.accounts.len(), "starting");

  let health = app
    .health_socket
    .as_ref()
    .map(|_| HealthState::new(app.accounts.iter().map(|a| a.name.clone())));

  let mut health_task = None;
  if let (Some(endpoint), Some(state)) = (app.health_socket.clone(), health.clone()) {
    let rx = shutdown_rx.clone();
    health_task = Some(tokio::spawn(async move {
      if let Err(err) = dka_runtime::serve(&endpoint, state, rx).await {
        error!(error = %err, "health server failed");
      }
    }));
  }

  let accounts = app
    .accounts
    .into_iter()
    .map(|account| adapt::session_params(account, &app.defaults))
    .collect();

  let supervisor = tokio::spawn(dka_runtime::run_accounts(
    accounts,
    health,
    shutdown_rx.clone(),
  ));

  while !*shutdown_rx.borrow_and_update() {
    if shutdown_rx.changed().await.is_err() {
      break;
    }
  }
  log_shutting_down(&reason_rx);

  let shutdown = async {
    if let Some(task) = health_task {
      let _ = task.await;
    }
    match supervisor.await {
      Ok(()) => {}
      Err(err) => {
        error!(error = %err, "supervisor failed");
        std::process::exit(1);
      }
    }
  };

  if tokio::time::timeout(SHUTDOWN_TIMEOUT, shutdown)
    .await
    .is_err()
  {
    error!(
      timeout_secs = SHUTDOWN_TIMEOUT.as_secs(),
      "shutdown timed out"
    );
    std::process::exit(1);
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

  dka_runtime::probe(&endpoint).await
}

fn spawn_shutdown_listener(
  shutdown_tx: watch::Sender<bool>,
  reason_tx: watch::Sender<Option<&'static str>>,
) {
  #[cfg(unix)]
  {
    use tokio::signal::unix::{SignalKind, signal};
    let mut sigterm = signal(SignalKind::terminate()).expect("register SIGTERM");
    let mut sigint = signal(SignalKind::interrupt()).expect("register SIGINT");
    tokio::spawn(async move {
      let reason = tokio::select! {
        _ = sigint.recv() => "SIGINT",
        _ = sigterm.recv() => "SIGTERM",
      };
      request_shutdown(&shutdown_tx, &reason_tx, reason);
    });
  }

  #[cfg(windows)]
  {
    use tokio::signal::windows::{ctrl_break, ctrl_c, ctrl_close};

    // Register CTRL handlers before spawn so startup cannot miss a signal.
    let mut ctrl_c = ctrl_c().expect("register CTRL+C handler");
    let mut ctrl_break = ctrl_break().expect("register CTRL+BREAK handler");
    let mut ctrl_close = ctrl_close().expect("register CTRL+CLOSE handler");
    tokio::spawn(async move {
      let reason = tokio::select! {
        _ = ctrl_c.recv() => "CTRL+C",
        _ = ctrl_break.recv() => "CTRL+BREAK",
        _ = ctrl_close.recv() => "CTRL+CLOSE",
      };
      request_shutdown(&shutdown_tx, &reason_tx, reason);
    });
  }

  #[cfg(not(any(unix, windows)))]
  {
    tokio::spawn(async move {
      let _ = tokio::signal::ctrl_c().await;
      request_shutdown(&shutdown_tx, &reason_tx, "SIGINT");
    });
  }
}

fn request_shutdown(
  shutdown_tx: &watch::Sender<bool>,
  reason_tx: &watch::Sender<Option<&'static str>>,
  reason: &'static str,
) {
  let _ = reason_tx.send(Some(reason));
  let _ = shutdown_tx.send(true);
}

fn log_shutting_down(reason_rx: &watch::Receiver<Option<&'static str>>) {
  let reason = reason_rx.borrow().unwrap_or("unknown");
  info!(reason, "shutting down");
}
