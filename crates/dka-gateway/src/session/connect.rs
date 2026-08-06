use std::sync::Arc;
use std::sync::atomic::AtomicI64;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::watch;
use tokio::time::Instant;
use tokio_tungstenite::{
  connect_async_with_config,
  tungstenite::{
    Message,
    client::IntoClientRequest,
    http::{HeaderValue, header::USER_AGENT},
  },
};
use tracing::{Instrument, Span, debug, error, warn};

use crate::compress::TransportDecompress;
use crate::heartbeat::{HeartbeatCmd, heartbeat_loop};
use crate::is_shutdown;
use crate::payload::{GatewayEnvelope, GatewayPayload, OP_HEARTBEAT, OP_RESUME};

use super::dispatch::{DispatchCtx, handle_payload};
use super::identify::{HelloWaitError, graceful_disconnect, send_identify, wait_for_hello};
use super::inbound::decode_inbound;
use super::{SessionEnd, SessionParams, SessionState, WsStream, send_json};

// Auth and shard/intent failures are fatal; do not reconnect.
fn is_fatal_close_code(code: u16) -> bool {
  matches!(code, 4004 | 4010..=4014)
}

// Invalid seq (4007): identify again, do not resume.
fn resume_after_close(code: u16) -> bool {
  code != 4007
}

pub(super) async fn connect_and_run(
  params: &SessionParams,
  state: &mut SessionState,
  url: &str,
  shutdown: &mut watch::Receiver<bool>,
) -> Result<SessionEnd> {
  if is_shutdown(shutdown) {
    return Ok(SessionEnd::Shutdown);
  }

  let user_agent = params.properties.user_agent.as_deref();
  let connect = connect_gateway(url, user_agent);
  tokio::pin!(connect);
  let (ws, _) = loop {
    tokio::select! {
      biased;
      changed = shutdown.changed() => {
        if changed.is_err() || is_shutdown(shutdown) {
          return Ok(SessionEnd::Shutdown);
        }
      }
      result = &mut connect => break result?,
    }
  };
  debug!("connected");
  let (mut write, mut read) = ws.split();
  let mut decomp = TransportDecompress::new()?;

  if is_shutdown(shutdown) {
    graceful_disconnect(&mut write, &mut read, false, params.kind).await;
    return Ok(SessionEnd::Shutdown);
  }

  let hello = match wait_for_hello(&mut read, &mut decomp, shutdown).await {
    Ok(hello) => hello,
    Err(HelloWaitError::Shutdown) => {
      graceful_disconnect(&mut write, &mut read, false, params.kind).await;
      return Ok(SessionEnd::Shutdown);
    }
    Err(HelloWaitError::Other(err)) => return Err(err),
  };
  let interval_ms = hello.heartbeat_interval;
  debug!(interval_ms, "hello received");

  let (hb_tx, mut hb_rx) = tokio::sync::mpsc::unbounded_channel::<HeartbeatCmd>();
  let (ack_tx, ack_rx) = watch::channel(Instant::now());
  let seq_cell = Arc::new(AtomicI64::new(state.seq.unwrap_or(-1)));

  let hb_seq = seq_cell.clone();
  let mut hb_shutdown = shutdown.clone();
  let hb_span = Span::current();
  let heartbeat_handle = tokio::spawn(
    async move {
      heartbeat_loop(interval_ms, hb_tx, ack_rx, hb_seq, &mut hb_shutdown).await;
    }
    .instrument(hb_span),
  );

  if state.can_resume() {
    let session_id = state.session_id.clone().unwrap();
    let seq = state.seq.unwrap();
    send_json(
      &mut write,
      &GatewayPayload::new(
        OP_RESUME,
        json!({
          "token": params.token,
          "session_id": session_id,
          "seq": seq,
        }),
      ),
    )
    .await?;
    debug!(seq, "resume sent");
  } else {
    send_identify(&mut write, params).await?;
    debug!("identify sent");
  }

  let mut presence_applied = false;
  let mut last_heartbeat_sent = None;
  let end: SessionEnd;

  loop {
    if is_shutdown(shutdown) {
      end = SessionEnd::Shutdown;
      break;
    }

    tokio::select! {
      biased;

      changed = shutdown.changed() => {
        if changed.is_err() || is_shutdown(shutdown) {
          end = SessionEnd::Shutdown;
          break;
        }
      }

      cmd = hb_rx.recv() => {
        match cmd {
          Some(HeartbeatCmd::Send { seq }) => {
            send_json(
              &mut write,
              &GatewayPayload::new(OP_HEARTBEAT, json!(seq)),
            )
            .await?;
            last_heartbeat_sent = Some(Instant::now());
            debug!(seq = %seq, "heartbeat sent");
          }
          Some(HeartbeatCmd::Zombie {
            interval_ms,
            since_ack_ms,
          }) => {
            warn!(interval_ms, since_ack_ms, "heartbeat ack timed out");
            end = SessionEnd::reconnect(true, "zombie");
            break;
          }
          None => {
            end = SessionEnd::reconnect(true, "heartbeat");
            break;
          }
        }
      }

      msg = read.next() => {
        match msg {
          Some(Ok(Message::Ping(data))) => {
            write.send(Message::Pong(data)).await?;
          }
          Some(Ok(Message::Close(frame))) => {
            let code = frame.as_ref().map(|f| u16::from(f.code)).unwrap_or(1000);
            let reason = frame
              .as_ref()
              .map(|f| f.reason.to_string())
              .unwrap_or_default();
            if is_fatal_close_code(code) {
              error!(code, reason = %reason, "gateway closed");
              end = SessionEnd::Fatal { code, reason };
            } else {
              let resume = resume_after_close(code);
              debug!(code, reason = %reason, resume, "connection closed");
              end = SessionEnd::reconnect(resume, "close");
            }
            break;
          }
          Some(Ok(msg)) => {
            let end_action = if let Some(text) = decode_inbound(msg, &mut decomp)? {
              let payload = GatewayEnvelope::from_json(text.as_str())
                .context("decode gateway payload")?;
              handle_payload(
                params,
                state,
                &payload,
                &mut DispatchCtx {
                  seq_cell: &seq_cell,
                  ack_tx: &ack_tx,
                  last_heartbeat_sent: &mut last_heartbeat_sent,
                  write: &mut write,
                  presence_applied: &mut presence_applied,
                },
              )
              .await?
            } else {
              None
            };
            decomp.reclaim();
            if let Some(session_end) = end_action {
              end = session_end;
              break;
            }
          }
          Some(Err(err)) => {
            heartbeat_handle.abort();
            return Err(err.into());
          }
          None => {
            end = SessionEnd::reconnect(true, "eof");
            break;
          }
        }
      }
    }
  }

  heartbeat_handle.abort();

  match &end {
    SessionEnd::Shutdown => {
      graceful_disconnect(&mut write, &mut read, presence_applied, params.kind).await;
    }
    SessionEnd::Reconnect { .. } | SessionEnd::Fatal { .. } => {
      let _ = write.close().await;
    }
  }

  Ok(end)
}

async fn connect_gateway(
  url: &str,
  user_agent: Option<&str>,
) -> Result<(
  WsStream,
  tokio_tungstenite::tungstenite::http::Response<Option<Vec<u8>>>,
)> {
  let mut request = url
    .into_client_request()
    .with_context(|| format!("build websocket request for {url}"))?;
  if let Some(ua) = user_agent.filter(|s| !s.is_empty()) {
    let value = HeaderValue::from_str(ua).context("invalid user-agent header value")?;
    request.headers_mut().insert(USER_AGENT, value);
  }
  connect_async_with_config(request, None, true)
    .await
    .with_context(|| format!("websocket connect to {url}"))
}
