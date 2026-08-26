use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
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
      // Give Discord time to apply presence before the session is closed.
      sleep(Duration::from_millis(250)).await;
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

pub(super) fn identify_d(params: &SessionParams) -> Value {
  // Identify carries presence for the pre-READY window; READY/RESUMED re-apply it.
  // Payload-level zlib off; transport already uses zstd-stream.
  let mut d = json!({
    "token": params.token,
    "properties": identify_properties(&params.properties, params.kind),
    "compress": false,
    "presence": params.presence.clone(),
  });
  if params.kind == AccountKind::Bot {
    d["large_threshold"] = json!(50);
    d["intents"] = json!(0);
  } else {
    d["large_threshold"] = json!(250);
    d["capabilities"] = json!(1_734_653);
    d["client_state"] = json!({
      "guild_versions": {},
      "api_code_version": 0,
    });
  }
  d
}

pub(super) async fn send_identify(write: &mut WsWrite, params: &SessionParams) -> Result<()> {
  send_json(write, &GatewayPayload::new(OP_IDENTIFY, identify_d(params))).await
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

#[cfg(test)]
mod tests {
  use dka_presence::AccountKind;
  use serde_json::json;

  use crate::properties::ClientProperties;
  use crate::session::SessionParams;

  use super::identify_d;

  fn params(kind: AccountKind, properties: ClientProperties) -> SessionParams {
    SessionParams {
      name: "t".into(),
      token: "tok".into(),
      kind,
      presence: json!({}),
      properties,
    }
  }

  #[test]
  fn user_identify_envelope() {
    let d = identify_d(&params(
      AccountKind::User,
      ClientProperties {
        os: "Windows".into(),
        browser: Some("Firefox".into()),
        device: String::new(),
        user_agent: Some("ua".into()),
        os_version: Some("10".into()),
        ..Default::default()
      },
    ));
    assert_eq!(d["token"], "tok");
    assert_eq!(d["compress"], false);
    assert_eq!(d["large_threshold"], 250);
    assert_eq!(d["presence"], json!({}));
    assert_eq!(d["capabilities"], 1_734_653);
    assert_eq!(
      d["client_state"],
      json!({"guild_versions": {}, "api_code_version": 0})
    );
    assert!(d.get("intents").is_none());
    let props = &d["properties"];
    assert_eq!(props["os"], "Windows");
    assert_eq!(props["has_client_mods"], false);
    assert!(props["client_event_source"].is_null());
    assert_eq!(props["browser_user_agent"], "ua");
    assert!(props.get("launch_signature").is_none());
  }

  #[test]
  fn bot_identify_envelope() {
    let d = identify_d(&params(
      AccountKind::Bot,
      ClientProperties {
        os: "linux".into(),
        browser: Some("discord-keep-alive".into()),
        device: "discord-keep-alive".into(),
        ..Default::default()
      },
    ));
    assert_eq!(d["token"], "tok");
    assert_eq!(d["compress"], false);
    assert_eq!(d["large_threshold"], 50);
    assert_eq!(d["intents"], 0);
    assert_eq!(d["presence"], json!({}));
    assert!(d.get("capabilities").is_none());
    assert!(d.get("client_state").is_none());
    let props = d["properties"].as_object().expect("properties");
    assert_eq!(props.len(), 3);
    assert_eq!(props["os"], "linux");
    assert_eq!(props["browser"], "discord-keep-alive");
    assert_eq!(props["device"], "discord-keep-alive");
  }
}
