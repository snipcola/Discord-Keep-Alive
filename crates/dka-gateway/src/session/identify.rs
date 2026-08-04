use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::sync::watch;
use tokio::time::sleep;
use tokio_tungstenite::tungstenite::{
  Message,
  protocol::{CloseFrame, frame::coding::CloseCode},
};
use tracing::{debug, trace};

use dka_presence::AccountKind;

use crate::compress::TransportDecompress;
use crate::is_shutdown;
use crate::payload::{
  GatewayEnvelope, GatewayPayload, HelloData, OP_HELLO, OP_IDENTIFY, OP_PRESENCE_UPDATE,
};
use crate::properties::identify_properties;

use super::inbound::decode_inbound;
use super::{SessionParams, WsRead, WsWrite, send_json};

pub(super) async fn graceful_disconnect(
  write: &mut WsWrite,
  read: &mut WsRead,
  send_offline_presence: bool,
  kind: AccountKind,
) {
  if send_offline_presence {
    let offline = dka_presence::build_presence_data(
      Some(dka_presence::Status::Invisible),
      None,
      &[],
      false,
      None,
      kind,
    );
    if let Err(err) = send_json(write, &GatewayPayload::new(OP_PRESENCE_UPDATE, offline)).await {
      debug!(error = %err, "failed to send offline presence");
    } else {
      trace!("sent offline presence");
    }
  }

  if let Err(err) = write
    .send(Message::Close(Some(CloseFrame {
      code: CloseCode::Normal,
      reason: "shutdown".into(),
    })))
    .await
  {
    debug!(error = %err, "failed to send close frame");
  } else {
    trace!("sent close frame");
  }

  let deadline = sleep(Duration::from_millis(750));
  tokio::pin!(deadline);
  loop {
    tokio::select! {
      _ = &mut deadline => break,
      msg = read.next() => {
        match msg {
          Some(Ok(Message::Close(frame))) => {
            trace!(?frame, "close handshake complete");
            break;
          }
          Some(Ok(Message::Ping(data))) => {
            let _ = write.send(Message::Pong(data)).await;
          }
          Some(Ok(_)) => {}
          Some(Err(_)) | None => break,
        }
      }
    }
  }

  let _ = write.close().await;
}

pub(super) async fn send_identify(write: &mut WsWrite, params: &SessionParams) -> Result<()> {
  // Identify carries presence for the pre-READY window; READY/RESUMED re-apply it.
  let mut d = json!({
    "token": params.token,
    "properties": identify_properties(&params.properties),
    // Payload-level zlib off; transport already uses zstd-stream.
    "compress": false,
    "large_threshold": 50,
    "presence": params.presence.clone(),
  });
  if params.kind == AccountKind::Bot {
    d["intents"] = json!(0);
  }
  send_json(write, &GatewayPayload::new(OP_IDENTIFY, d)).await
}

pub(super) enum HelloWaitError {
  Shutdown,
  Other(anyhow::Error),
}

impl HelloWaitError {
  fn other(err: impl Into<anyhow::Error>) -> Self {
    Self::Other(err.into())
  }

  fn closed() -> Self {
    Self::Other(anyhow::anyhow!("connection closed before Hello"))
  }
}

impl From<anyhow::Error> for HelloWaitError {
  fn from(err: anyhow::Error) -> Self {
    Self::Other(err)
  }
}

pub(super) async fn wait_for_hello(
  read: &mut WsRead,
  decomp: &mut TransportDecompress,
  shutdown: &mut watch::Receiver<bool>,
) -> Result<HelloData, HelloWaitError> {
  let deadline = sleep(Duration::from_secs(30));
  tokio::pin!(deadline);

  loop {
    if is_shutdown(shutdown) {
      return Err(HelloWaitError::Shutdown);
    }

    tokio::select! {
      _ = &mut deadline => {
        return Err(HelloWaitError::other(anyhow::anyhow!("timed out waiting for Hello")));
      }
      changed = shutdown.changed() => {
        if changed.is_err() || is_shutdown(shutdown) {
          return Err(HelloWaitError::Shutdown);
        }
      }
      msg = read.next() => {
        match msg {
          Some(Ok(Message::Ping(_))) => {}
          Some(Ok(Message::Close(_))) | None => return Err(HelloWaitError::closed()),
          Some(Ok(msg)) => {
            let hello = if let Some(text) = decode_inbound(msg, decomp)? {
              let payload = GatewayEnvelope::from_json(text.as_str())
                .context("decode gateway payload")?;
              if payload.op == OP_HELLO {
                let d = payload.d_str().unwrap_or("null");
                Some(serde_json::from_str::<HelloData>(d).context("decode Hello data")?)
              } else {
                None
              }
            } else {
              None
            };
            decomp.reclaim();
            if let Some(hello) = hello {
              return Ok(hello);
            }
          }
          Some(Err(err)) => return Err(HelloWaitError::other(err)),
        }
      }
    }
  }
}
