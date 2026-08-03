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

use crate::payload::{GatewayPayload, HelloData, OP_HELLO, OP_IDENTIFY, OP_PRESENCE_UPDATE};
use crate::properties::identify_properties;

use super::{SessionParams, WsRead, WsWrite, send_json};

/// Best-effort shutdown: optional offline presence, then close handshake.
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
  // Presence is re-sent on READY so the same finished payload is applied again.
  let mut d = json!({
    "token": params.token,
    "properties": identify_properties(&params.properties),
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

impl From<anyhow::Error> for HelloWaitError {
  fn from(err: anyhow::Error) -> Self {
    Self::Other(err)
  }
}

impl From<tokio_tungstenite::tungstenite::Error> for HelloWaitError {
  fn from(err: tokio_tungstenite::tungstenite::Error) -> Self {
    Self::Other(err.into())
  }
}

impl From<serde_json::Error> for HelloWaitError {
  fn from(err: serde_json::Error) -> Self {
    Self::Other(err.into())
  }
}

pub(super) async fn wait_for_hello(
  read: &mut WsRead,
  shutdown: &mut watch::Receiver<bool>,
) -> Result<HelloData, HelloWaitError> {
  let deadline = sleep(Duration::from_secs(30));
  tokio::pin!(deadline);

  loop {
    if *shutdown.borrow() {
      return Err(HelloWaitError::Shutdown);
    }

    tokio::select! {
      _ = &mut deadline => return Err(HelloWaitError::Other(anyhow::anyhow!("timed out waiting for Hello"))),
      changed = shutdown.changed() => {
        if changed.is_err() || *shutdown.borrow() {
          return Err(HelloWaitError::Shutdown);
        }
      }
      msg = read.next() => {
        match msg {
          Some(Ok(Message::Text(text))) => {
            let payload = GatewayPayload::from_json(&text)?;
            if payload.op == OP_HELLO {
              let hello: HelloData = serde_json::from_value(payload.d)
                .context("decode Hello data")?;
              return Ok(hello);
            }
          }
          Some(Ok(Message::Ping(_))) => {}
          Some(Ok(Message::Close(_))) => {
            return Err(HelloWaitError::Other(anyhow::anyhow!(
              "connection closed before Hello"
            )));
          }
          Some(Err(err)) => return Err(err.into()),
          None => {
            return Err(HelloWaitError::Other(anyhow::anyhow!(
              "connection closed before Hello"
            )));
          }
          _ => {}
        }
      }
    }
  }
}
