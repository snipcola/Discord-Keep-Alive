use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use anyhow::Result;
use serde_json::json;
use tokio::sync::watch;
use tokio::time::Instant;
use tracing::{debug, info, trace, warn};

use crate::config::AccountConfig;
use crate::gateway::payload::{
  GatewayPayload, OP_DISPATCH, OP_HEARTBEAT, OP_HEARTBEAT_ACK, OP_HELLO, OP_INVALID_SESSION,
  OP_PRESENCE_UPDATE, OP_RECONNECT, ReadyInfo,
};
use crate::gateway::presence::build_and_log_presence;

use super::{SessionState, WsWrite, send_json};

pub(super) enum PayloadAction {
  Continue,
  Reconnect {
    resume: bool,
    extra_delay: Option<Duration>,
  },
}

pub(super) async fn handle_payload(
  account: &AccountConfig,
  state: &mut SessionState,
  payload: &GatewayPayload,
  seq_cell: &Arc<AtomicI64>,
  ack_tx: &watch::Sender<Instant>,
  write: &mut WsWrite,
  presence_applied: &mut bool,
) -> Result<PayloadAction> {
  if let Some(s) = payload.s {
    state.seq = Some(s);
    seq_cell.store(s, Ordering::Relaxed);
  }

  match payload.op {
    OP_HELLO => Ok(PayloadAction::Continue),
    OP_HEARTBEAT_ACK => {
      let _ = ack_tx.send(Instant::now());
      Ok(PayloadAction::Continue)
    }
    OP_HEARTBEAT => {
      send_json(write, &GatewayPayload::new(OP_HEARTBEAT, json!(state.seq))).await?;
      Ok(PayloadAction::Continue)
    }
    OP_RECONNECT => {
      warn!("server requested reconnect");
      Ok(PayloadAction::Reconnect {
        resume: true,
        extra_delay: None,
      })
    }
    OP_INVALID_SESSION => {
      let resumable = payload.d.as_bool().unwrap_or(false);
      warn!(resumable, "invalid session");
      if !resumable {
        state.clear_session();
      }
      Ok(PayloadAction::Reconnect {
        resume: resumable,
        extra_delay: Some(Duration::from_secs(2)),
      })
    }
    OP_DISPATCH => {
      let event = payload.t.as_deref().unwrap_or("");
      match event {
        "READY" => {
          if let Some(info) = ReadyInfo::from_ready_data(&payload.d) {
            state.session_id = Some(info.session_id.clone());
            state.resume_url = Some(info.resume_gateway_url.clone());
            state.set_healthy(true);
            info!("logged in as {}", info.display_name());
          } else {
            state.set_healthy(true);
            info!("logged in");
          }
          apply_presence(account, write, presence_applied).await?;
          Ok(PayloadAction::Continue)
        }
        "RESUMED" => {
          state.set_healthy(true);
          info!("session resumed");
          if !*presence_applied {
            apply_presence(account, write, presence_applied).await?;
          }
          Ok(PayloadAction::Continue)
        }
        _ => {
          trace!(event, "dispatch");
          Ok(PayloadAction::Continue)
        }
      }
    }
    other => {
      debug!(op = other, "unhandled opcode");
      Ok(PayloadAction::Continue)
    }
  }
}

async fn apply_presence(
  account: &AccountConfig,
  write: &mut WsWrite,
  presence_applied: &mut bool,
) -> Result<()> {
  let data = build_and_log_presence(account);
  send_json(write, &GatewayPayload::new(OP_PRESENCE_UPDATE, data)).await?;
  *presence_applied = true;
  Ok(())
}
