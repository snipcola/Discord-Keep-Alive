use anyhow::{Context, Result};
use tokio_tungstenite::tungstenite::Message;

use crate::compress::TransportDecompress;
use crate::payload::GatewayPayload;

/// `Ok(None)` for control frames (ping/pong/close/etc.) handled by the caller.
pub(super) fn decode_inbound(
  msg: Message,
  decomp: &mut TransportDecompress,
) -> Result<Option<GatewayPayload>> {
  match msg {
    Message::Text(text) => GatewayPayload::from_json(&text)
      .context("decode gateway payload")
      .map(Some),
    Message::Binary(data) => {
      let json = decomp.push(&data)?;
      let text = std::str::from_utf8(json).context("utf-8 after zstd-stream decompress")?;
      GatewayPayload::from_json(text)
        .context("decode gateway payload")
        .map(Some)
    }
    _ => Ok(None),
  }
}
