mod connect;
mod dispatch;
mod identify;
mod inbound;

use std::time::Duration;

use anyhow::Result;
use dka_presence::AccountKind;
use futures_util::stream::{SplitSink, SplitStream};
use serde_json::Value;
use tokio::sync::watch;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::{
  Message,
  protocol::{CloseFrame, frame::coding::CloseCode},
};
use tracing::{debug, error, info};

use crate::payload::GatewayPayload;
use crate::properties::ClientProperties;
use crate::reconnect::{backoff_with_jitter, resume_ws_url};
use crate::{GATEWAY_HOST, gateway_url, is_shutdown};

use self::connect::connect_and_run;

type WsStream =
  tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>>;
type WsWrite = SplitSink<WsStream, Message>;
type WsRead = SplitStream<WsStream>;

// Presence is ready-made JSON; the gateway never rebuilds it.
#[derive(Debug, Clone)]
pub struct SessionParams {
  pub name: String,
  pub token: String,
  pub kind: AccountKind,
  pub presence: Value,
  pub properties: ClientProperties,
}

pub type LiveSink = Box<dyn FnMut(bool) + Send>;

struct SessionState {
  on_live: LiveSink,
  seq: Option<i64>,
  session_id: Option<String>,
  resume_url: Option<String>,
  // Set after READY/RESUMED. A healthy drop resets reconnect attempts.
  session_healthy: bool,
}

impl SessionState {
  fn new(on_live: LiveSink) -> Self {
    Self {
      on_live,
      seq: None,
      session_id: None,
      resume_url: None,
      session_healthy: false,
    }
  }

  fn can_resume(&self) -> bool {
    self.session_id.is_some() && self.seq.is_some()
  }

  // Only resume a session that reached READY/RESUMED; a resume that died earlier
  // would just be retried against a session the gateway already rejected.
  fn can_resume_after_error(&self) -> bool {
    self.session_healthy && self.can_resume()
  }

  fn clear_session(&mut self) {
    self.session_id = None;
    self.resume_url = None;
    self.seq = None;
    self.set_healthy(false);
  }

  fn set_healthy(&mut self, healthy: bool) {
    self.session_healthy = healthy;
    (self.on_live)(healthy);
  }
}

const RECONNECT_CLOSE_CODE: u16 = 4000;

enum SessionEnd {
  Shutdown,
  Reconnect {
    resume: bool,
    // Optional extra wait (for example 2s after INVALID_SESSION).
    extra_delay: Option<Duration>,
    cause: &'static str,
  },
  Fatal {
    code: u16,
    reason: String,
  },
}

impl SessionEnd {
  fn reconnect(resume: bool, cause: &'static str) -> Self {
    Self::Reconnect {
      resume,
      extra_delay: None,
      cause,
    }
  }

  fn reconnect_after(resume: bool, delay: Duration, cause: &'static str) -> Self {
    Self::Reconnect {
      resume,
      extra_delay: Some(delay),
      cause,
    }
  }

  // 1000/1001 invalidate the session server-side; any other code keeps it resumable.
  fn close_frame(&self) -> Option<CloseFrame> {
    match self {
      Self::Reconnect { .. } => Some(CloseFrame {
        code: CloseCode::from(RECONNECT_CLOSE_CODE),
        reason: "reconnecting".into(),
      }),
      Self::Shutdown | Self::Fatal { .. } => None,
    }
  }
}

pub async fn run_session(
  params: SessionParams,
  on_live: LiveSink,
  mut shutdown: watch::Receiver<bool>,
) -> Result<()> {
  let mut state = SessionState::new(on_live);
  let mut attempt: u32 = 0;

  loop {
    if is_shutdown(&shutdown) {
      return finish_disconnect(&mut state);
    }

    let connect_url = if state.can_resume() {
      resume_ws_url(state.resume_url.as_deref().unwrap_or(GATEWAY_HOST))
    } else {
      gateway_url()
    };

    debug!("connecting to {}", display_ws_url(&connect_url));
    state.set_healthy(false);

    let (resume, extra_delay, cause) =
      match connect_and_run(&params, &mut state, &connect_url, &mut shutdown).await {
        Ok(SessionEnd::Shutdown) => return finish_disconnect(&mut state),
        Ok(SessionEnd::Fatal { code, reason }) => {
          state.set_healthy(false);
          error!(code, reason = %reason, "session stopped (fatal close)");
          return Ok(());
        }
        Ok(SessionEnd::Reconnect {
          resume,
          extra_delay,
          cause,
        }) => (resume, extra_delay, cause),
        Err(err) => {
          error!(error = %err, "session failed");
          (state.can_resume_after_error(), None, "error")
        }
      };

    if schedule_reconnect(
      &mut state,
      &mut attempt,
      resume,
      extra_delay,
      cause,
      &mut shutdown,
    )
    .await
    {
      return Ok(());
    }
  }
}

fn finish_disconnect(state: &mut SessionState) -> Result<()> {
  state.set_healthy(false);
  info!("disconnected");
  Ok(())
}

async fn schedule_reconnect(
  state: &mut SessionState,
  attempt: &mut u32,
  resume: bool,
  extra_delay: Option<Duration>,
  cause: &str,
  shutdown: &mut watch::Receiver<bool>,
) -> bool {
  let was_healthy = state.session_healthy;
  if resume {
    state.set_healthy(false);
  } else {
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
    "reconnecting in {} (attempt {n}, resume={resume}, cause={cause})",
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
  if is_shutdown(shutdown) {
    return true;
  }
  tokio::select! {
    _ = sleep(delay) => false,
    changed = shutdown.changed() => changed.is_err() || is_shutdown(shutdown),
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

#[cfg(test)]
mod tests {
  use super::*;

  fn state(healthy: bool, session: bool) -> SessionState {
    let mut state = SessionState::new(Box::new(|_| {}));
    if session {
      state.session_id = Some("abc".into());
      state.seq = Some(7);
    }
    state.session_healthy = healthy;
    state
  }

  #[test]
  fn error_resumes_only_once_session_is_established() {
    assert!(state(true, true).can_resume_after_error());
    assert!(!state(false, true).can_resume_after_error());
    assert!(!state(true, false).can_resume_after_error());
  }

  #[test]
  fn only_reconnect_closes_and_keeps_session_resumable() {
    let frame = SessionEnd::reconnect(true, "reconnect")
      .close_frame()
      .expect("reconnect sends a close frame");
    assert!(!matches!(u16::from(frame.code), 1000 | 1001));
    assert!(frame.code.is_allowed());

    assert!(SessionEnd::Shutdown.close_frame().is_none());
    assert!(
      SessionEnd::Fatal {
        code: 4004,
        reason: "auth".into(),
      }
      .close_frame()
      .is_none()
    );
  }
}
