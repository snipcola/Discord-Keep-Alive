use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use anyhow::Result;
use serde_json::json;
use tokio::sync::watch;
use tokio::time::Instant;
use tracing::{debug, info, trace, warn};

use crate::payload::{
  GatewayEnvelope, GatewayPayload, OP_DISPATCH, OP_HEARTBEAT, OP_HEARTBEAT_ACK, OP_HELLO,
  OP_INVALID_SESSION, OP_PRESENCE_UPDATE, OP_RECONNECT, ReadyInfo,
};

use super::{SessionEnd, SessionParams, SessionState, WsWrite, send_json};

pub(super) async fn handle_payload(
  params: &SessionParams,
  state: &mut SessionState,
  payload: &GatewayEnvelope<'_>,
  seq_cell: &Arc<AtomicI64>,
  ack_tx: &watch::Sender<Instant>,
  write: &mut WsWrite,
  presence_applied: &mut bool,
) -> Result<Option<SessionEnd>> {
  if let Some(s) = payload.s {
    state.seq = Some(s);
    seq_cell.store(s, Ordering::Relaxed);
  }

  Ok(match payload.op {
    OP_HELLO => None,
    OP_HEARTBEAT_ACK => {
      let _ = ack_tx.send(Instant::now());
      None
    }
    OP_HEARTBEAT => {
      send_json(write, &GatewayPayload::new(OP_HEARTBEAT, json!(state.seq))).await?;
      None
    }
    OP_RECONNECT => {
      warn!("server requested reconnect");
      Some(SessionEnd::reconnect(true))
    }
    OP_INVALID_SESSION => {
      let resumable = payload
        .d_str()
        .and_then(|d| serde_json::from_str(d).ok())
        .unwrap_or(false);
      warn!(resumable, "invalid session");
      if !resumable {
        state.clear_session();
      }
      Some(SessionEnd::reconnect_after(
        resumable,
        Duration::from_secs(2),
      ))
    }
    OP_DISPATCH => {
      let event = payload.t.unwrap_or("");
      match event {
        "READY" => {
          if let Some(info) = payload.d_str().and_then(ReadyInfo::from_ready_json) {
            state.session_id = Some(info.session_id.clone());
            state.resume_url = Some(info.resume_gateway_url.clone());
            state.set_healthy(true);
            info!("logged in as {}", info.display_name());
          } else {
            state.set_healthy(true);
            info!("logged in");
          }
          apply_presence(params, write, presence_applied).await?;
          None
        }
        "RESUMED" => {
          state.set_healthy(true);
          info!("session resumed");
          if !*presence_applied {
            apply_presence(params, write, presence_applied).await?;
          }
          None
        }
        _ => {
          trace!(event, "dispatch");
          None
        }
      }
    }
    other => {
      debug!(op = other, "unhandled opcode");
      None
    }
  })
}

async fn apply_presence(
  params: &SessionParams,
  write: &mut WsWrite,
  presence_applied: &mut bool,
) -> Result<()> {
  send_json(
    write,
    &GatewayPayload::new(OP_PRESENCE_UPDATE, params.presence.clone()),
  )
  .await?;
  *presence_applied = true;
  Ok(())
}
