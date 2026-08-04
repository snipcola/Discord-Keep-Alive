use anyhow::{Context, Result};
use tokio_tungstenite::tungstenite::{Message, protocol::frame::Utf8Bytes};

use crate::compress::TransportDecompress;

// Decompressed text is only valid until the next decompress push.
pub(super) enum InboundText<'a> {
  Text(Utf8Bytes),
  Decompressed(&'a str),
}

impl<'a> InboundText<'a> {
  pub fn as_str(&self) -> &str {
    match self {
      Self::Text(s) => s.as_ref(),
      Self::Decompressed(s) => s,
    }
  }
}

// Returns None for control frames (ping/pong/close); the caller handles those.
pub(super) fn decode_inbound<'a>(
  msg: Message,
  decomp: &'a mut TransportDecompress,
) -> Result<Option<InboundText<'a>>> {
  match msg {
    Message::Text(text) => Ok(Some(InboundText::Text(text))),
    Message::Binary(data) => {
      let json = decomp.push(&data)?;
      let text = std::str::from_utf8(json).context("utf-8 after zstd-stream decompress")?;
      Ok(Some(InboundText::Decompressed(text)))
    }
    _ => Ok(None),
  }
}
