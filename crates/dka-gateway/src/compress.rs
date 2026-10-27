use anyhow::{Context, Result};
use structured_zstd::decoding::FrameDecoder;

const INITIAL_LEN: usize = 8 * 1024;
// Capacity kept after reclaiming, enough for normal frames.
const KEEP_CAP: usize = 64 * 1024;
// The buffer only shrinks after a large spike, such as READY.
const SHRINK_THRESHOLD: usize = 256 * 1024;

// One zstd stream per WebSocket connection.
pub(crate) struct TransportDecompress {
  decoder: FrameDecoder,
  pending: Vec<u8>,
  out: Vec<u8>,
  out_len: usize,
}

impl TransportDecompress {
  pub(crate) fn new() -> Self {
    Self {
      decoder: FrameDecoder::new(),
      pending: Vec::new(),
      out: vec![0; INITIAL_LEN],
      out_len: 0,
    }
  }

  pub(crate) fn push(&mut self, compressed: &[u8]) -> Result<&[u8]> {
    self.out_len = 0;

    if self.pending.is_empty() {
      let used = self.decode(compressed)?;
      self.pending.extend_from_slice(&compressed[used..]);
    } else {
      let mut input = std::mem::take(&mut self.pending);
      input.extend_from_slice(compressed);
      let used = self.decode(&input)?;
      input.drain(..used);
      self.pending = input;
    }

    Ok(&self.out[..self.out_len])
  }

  fn decode(&mut self, input: &[u8]) -> Result<usize> {
    let mut used = 0;

    loop {
      if self.out_len == self.out.len() {
        self.out.resize(self.out.len() * 2, 0);
      }

      let (read, written) = self
        .decoder
        .decode_from_to(&input[used..], &mut self.out[self.out_len..])
        .context("zstd-stream decompress")?;

      used += read;
      self.out_len += written;

      if read == 0 && written == 0 {
        return Ok(used);
      }
    }
  }

  pub(crate) fn reclaim(&mut self) {
    if self.out.len() > SHRINK_THRESHOLD {
      self.out_len = 0;
      self.out.truncate(KEEP_CAP);
      self.out.shrink_to_fit();
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use std::io::Write;
  use structured_zstd::encoding::{CompressionLevel, StreamingEncoder};

  fn compress_stream_messages(chunks: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut encoder = StreamingEncoder::new(Vec::new(), CompressionLevel::Default);

    chunks
      .iter()
      .map(|chunk| {
        encoder.write_all(chunk).expect("compress");
        encoder.flush().expect("flush");
        std::mem::take(encoder.get_mut())
      })
      .collect()
  }

  #[test]
  fn decompresses_multi_message_stream() {
    let a = br#"{"op":10,"d":{"heartbeat_interval":41250}}"#;
    let b = br#"{"op":11}"#;
    let frames = compress_stream_messages(&[a, b]);
    assert_eq!(frames.len(), 2);
    assert!(!frames[0].is_empty());
    assert!(!frames[1].is_empty());

    let mut decomp = TransportDecompress::new();
    let out_a = decomp.push(&frames[0]).unwrap().to_vec();
    assert_eq!(out_a, a);
    let out_b = decomp.push(&frames[1]).unwrap().to_vec();
    assert_eq!(out_b, b);
    assert!(decomp.pending.is_empty());
  }

  #[test]
  fn decompresses_output_larger_than_buffer() {
    let big: Vec<u8> = (0..INITIAL_LEN * 8).map(|i| (i % 251) as u8).collect();
    let frames = compress_stream_messages(&[&big, b"{}"]);

    let mut decomp = TransportDecompress::new();
    assert_eq!(decomp.push(&frames[0]).unwrap(), big.as_slice());
    assert_eq!(decomp.push(&frames[1]).unwrap(), b"{}");
  }

  #[test]
  fn decompresses_blocks_split_across_pushes() {
    let msgs: Vec<Vec<u8>> = (0..20)
      .map(|i| format!(r#"{{"op":0,"s":{i},"d":"{}"}}"#, "x".repeat(i * 300)).into_bytes())
      .collect();
    let refs: Vec<&[u8]> = msgs.iter().map(Vec::as_slice).collect();
    let stream = compress_stream_messages(&refs).concat();

    let mut decomp = TransportDecompress::new();
    let mut out = Vec::new();
    for chunk in stream.chunks(7) {
      out.extend_from_slice(decomp.push(chunk).unwrap());
    }
    assert_eq!(out, msgs.concat());
    assert!(decomp.pending.is_empty());
  }

  #[test]
  fn reclaim_shrinks_large_capacity() {
    let mut d = TransportDecompress::new();
    d.out.resize(2 * 1024 * 1024, 0);
    d.out_len = d.out.len();
    let before = d.out.capacity();
    d.reclaim();
    assert_eq!(d.out_len, 0);
    assert_eq!(d.out.len(), KEEP_CAP);
    assert!(d.out.capacity() <= SHRINK_THRESHOLD);
    assert!(d.out.capacity() < before);
  }

  #[test]
  fn reclaim_noop_when_capacity_small() {
    let mut d = TransportDecompress::new();
    let before = d.out.len();
    assert!(before <= SHRINK_THRESHOLD);
    d.reclaim();
    assert_eq!(d.out.len(), before);
  }
}
