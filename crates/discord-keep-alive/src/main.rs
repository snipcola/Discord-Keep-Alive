mod adapt;
mod log;
mod term;

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

  if matches!(cli.command, Some(Command::Health)) {
    std::process::exit(run_health_probe(&cli).await);
  }

  let (shutdown_tx, mut shutdown_rx) = watch::channel(false);
  let (reason_tx, reason_rx) = watch::channel(None::<&'static str>);
  spawn_shutdown_listener(shutdown_tx.clone(), reason_tx);

  rustls::crypto::ring::default_provider()
    .install_default()
    .expect("install rustls CryptoProvider");

  let app = match load(&cli) {
    Ok(cfg) => cfg,
    Err(err) => {
      term::report_error(err);
      std::process::exit(1);
    }
  };

  log::init(&app.log_level);

  if *shutdown_rx.borrow() {
    log_shutting_down(&reason_rx);
    return;
  }

  let count = app.accounts.len();
  info!(
    "starting {count} account{}",
    if count == 1 { "" } else { "s" }
  );

  let health = app
    .health_socket
    .as_ref()
    .map(|_| HealthState::new(app.accounts.iter().map(|a| a.name.clone())));

  let mut health_task = None;
  if let (Some(endpoint), Some(state)) = (app.health_socket.clone(), health.clone()) {
    let rx = shutdown_rx.clone();
    health_task = Some(tokio::spawn(async move {
      if let Err(err) = dka_runtime::serve(&endpoint, state, rx).await {
        error!("health server failed: {err:#}");
      }
    }));
  }

  let accounts = app
    .accounts
    .into_iter()
    .map(|account| adapt::session_params(account, &app.defaults))
    .collect();

  let mut supervisor = tokio::spawn(dka_runtime::run_accounts(
    accounts,
    health,
    shutdown_rx.clone(),
  ));

  let early_result = tokio::select! {
    biased;
    () = wait_for_shutdown(&mut shutdown_rx) => {
      log_shutting_down(&reason_rx);
      None
    }
    result = &mut supervisor => {
      let _ = shutdown_tx.send(true);
      Some(result)
    }
  };
  let accounts_stopped = early_result.is_some();

  let shutdown = async {
    if let Some(task) = health_task {
      let _ = task.await;
    }
    match early_result {
      Some(result) => result,
      None => supervisor.await,
    }
  };

  let exit_code = match tokio::time::timeout(SHUTDOWN_TIMEOUT, shutdown).await {
    Err(_) => {
      error!(
        timeout_secs = SHUTDOWN_TIMEOUT.as_secs(),
        "shutdown timed out"
      );
      1
    }
    Ok(Err(err)) => {
      error!("supervisor failed: {err}");
      1
    }
    Ok(Ok(())) if accounts_stopped => {
      error!("all accounts stopped");
      1
    }
    Ok(Ok(())) => 0,
  };
  if exit_code != 0 {
    std::process::exit(exit_code);
  }
}

async fn run_health_probe(cli: &Cli) -> i32 {
  let endpoint = match load_health_endpoint(&cli.config, cli.health_socket.as_deref()) {
    Ok(Some(endpoint)) => endpoint,
    Ok(None) => {
      term::report_error(
        "health socket not configured (set health_socket, HEALTH_SOCKET or --health-socket)",
      );
      return 2;
    }
    Err(err) => {
      term::report_error(err);
      return 2;
    }
  };

  match dka_runtime::probe(&endpoint).await {
    Ok(()) => 0,
    Err(err) => {
      term::report_error(&err);
      err.exit_code()
    }
  }
}

async fn wait_for_shutdown(shutdown_rx: &mut watch::Receiver<bool>) {
  while !*shutdown_rx.borrow_and_update() {
    if shutdown_rx.changed().await.is_err() {
      break;
    }
  }
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

    // Register the CTRL handlers before spawning, so startup can't miss a signal.
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
  match *reason_rx.borrow() {
    Some(reason) => info!("received {reason}, shutting down"),
    None => info!("shutting down"),
  }
}
