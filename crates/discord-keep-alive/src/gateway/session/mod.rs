mod connect;
mod dispatch;
mod identify;

use std::time::Duration;

use anyhow::Result;
use futures_util::stream::{SplitSink, SplitStream};
use tokio::sync::watch;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::Message;
use tracing::{debug, error, info};

use crate::config::AccountConfig;
use crate::gateway::payload::GatewayPayload;
use crate::gateway::properties::Defaults;
use crate::gateway::reconnect::{backoff_with_jitter, resume_ws_url};
use crate::gateway::{GATEWAY_HOST, gateway_url};

use self::connect::connect_and_run;

type WsStream =
  tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
type WsWrite = SplitSink<WsStream, Message>;
type WsRead = SplitStream<WsStream>;

struct SessionState {
  seq: Option<i64>,
  session_id: Option<String>,
  resume_url: Option<String>,
  /// True after READY/RESUMED; used to reset reconnect backoff.
  session_healthy: bool,
}

impl SessionState {
  fn new() -> Self {
    Self {
      seq: None,
      session_id: None,
      resume_url: None,
      session_healthy: false,
    }
  }

  fn can_resume(&self) -> bool {
    self.session_id.is_some() && self.seq.is_some()
  }

  fn clear_session(&mut self) {
    self.session_id = None;
    self.resume_url = None;
    self.seq = None;
    self.session_healthy = false;
  }
}

enum SessionEnd {
  Shutdown,
  Reconnect {
    resume: bool,
    /// Extra wait before backoff (e.g. after INVALID_SESSION).
    extra_delay: Option<Duration>,
  },
  Fatal {
    code: u16,
    reason: String,
  },
}

pub async fn run_session(
  account: AccountConfig,
  defaults: Defaults,
  mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
  let mut state = SessionState::new();
  let mut attempt: u32 = 0;

  loop {
    if *shutdown.borrow() {
      info!("disconnected");
      return Ok(());
    }

    let connect_url = if state.can_resume() {
      resume_ws_url(state.resume_url.as_deref().unwrap_or(GATEWAY_HOST))
    } else {
      gateway_url()
    };

    debug!("connecting to {}", display_ws_url(&connect_url));
    state.session_healthy = false;

    match connect_and_run(&account, &defaults, &mut state, &connect_url, &mut shutdown).await {
      Ok(SessionEnd::Shutdown) => {
        info!("disconnected");
        return Ok(());
      }
      Ok(SessionEnd::Fatal { code, reason }) => {
        error!(code, reason = %reason, "session stopped (fatal close)");
        return Ok(());
      }
      Ok(SessionEnd::Reconnect {
        resume,
        extra_delay,
      }) => {
        if schedule_reconnect(
          &mut state,
          &mut attempt,
          !resume,
          resume,
          extra_delay,
          &mut shutdown,
        )
        .await
        {
          return Ok(());
        }
      }
      Err(err) => {
        error!(error = %err, "session failed");
        if schedule_reconnect(&mut state, &mut attempt, true, false, None, &mut shutdown).await {
          return Ok(());
        }
      }
    }
  }
}

async fn schedule_reconnect(
  state: &mut SessionState,
  attempt: &mut u32,
  clear_session: bool,
  resume: bool,
  extra_delay: Option<Duration>,
  shutdown: &mut watch::Receiver<bool>,
) -> bool {
  let was_healthy = state.session_healthy;
  if clear_session {
    state.clear_session();
  }
  if was_healthy {
    *attempt = 0;
  }
  *attempt = attempt.saturating_add(1);
  let mut delay = backoff_with_jitter(*attempt);
  if let Some(extra) = extra_delay {
    delay = delay.saturating_add(extra);
  }
  let n = *attempt;
  info!(
    "reconnecting in {} (attempt {n}, resume={resume})",
    format_delay(delay)
  );
  if wait_or_shutdown(delay, shutdown).await {
    info!("disconnected");
    true
  } else {
    false
  }
}

fn display_ws_url(url: &str) -> &str {
  let without_query = url.split_once('?').map(|(base, _)| base).unwrap_or(url);
  without_query.trim_end_matches('/')
}

fn format_delay(delay: Duration) -> String {
  let ms = delay.as_millis();
  if ms.is_multiple_of(1000) {
    format!("{}s", ms / 1000)
  } else {
    format!("{:.1}s", ms as f64 / 1000.0)
  }
}

async fn wait_or_shutdown(delay: Duration, shutdown: &mut watch::Receiver<bool>) -> bool {
  if *shutdown.borrow() {
    return true;
  }
  tokio::select! {
    _ = sleep(delay) => false,
    _ = shutdown.changed() => *shutdown.borrow(),
  }
}

async fn send_json(write: &mut WsWrite, payload: &GatewayPayload) -> Result<()> {
  use anyhow::Context;
  use futures_util::SinkExt;
  let text = payload.to_json()?;
  write
    .send(Message::Text(text.into()))
    .await
    .context("send websocket text")?;
  Ok(())
}
