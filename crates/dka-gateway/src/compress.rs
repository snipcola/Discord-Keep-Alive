use anyhow::{Context, Result};
use zstd::stream::raw::{Decoder, InBuffer, Operation, OutBuffer};

const SCRATCH_LEN: usize = 8 * 1024;
// Keep this much capacity after reclaim for normal frames.
const KEEP_CAP: usize = 64 * 1024;
// Only shrink after large spikes (for example READY).
const SHRINK_THRESHOLD: usize = 256 * 1024;

// Shared zstd stream for the life of one WebSocket connection.
pub(crate) struct TransportDecompress {
  decoder: Decoder<'static>,
  out: Vec<u8>,
  scratch: Vec<u8>,
}

impl TransportDecompress {
  pub(crate) fn new() -> Result<Self> {
    Ok(Self {
      decoder: Decoder::new().context("create zstd-stream decoder")?,
      out: Vec::with_capacity(SCRATCH_LEN),
      scratch: vec![0; SCRATCH_LEN],
    })
  }

  // Output is only valid until the next push.
  pub(crate) fn push(&mut self, compressed: &[u8]) -> Result<&[u8]> {
    self.out.clear();
    let mut input = InBuffer::around(compressed);

    loop {
      let mut output = OutBuffer::around(self.scratch.as_mut_slice());
      self
        .decoder
        .run(&mut input, &mut output)
        .context("zstd-stream decompress")?;

      self.out.extend_from_slice(output.as_slice());

      let in_done = input.pos() >= compressed.len();
      let out_full = output.pos() >= self.scratch.len();
      // Keep reading while the scratch buffer is full.
      if in_done && !out_full {
        break;
      }
    }

    Ok(&self.out)
  }

  // Clear before shrink_to; shrink cannot drop below current len.
  pub(crate) fn reclaim(&mut self) {
    if self.out.capacity() > SHRINK_THRESHOLD {
      self.out.clear();
      self.out.shrink_to(KEEP_CAP);
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use zstd::stream::raw::{Encoder, InBuffer, Operation, OutBuffer};

  // One continuous zstd stream with compress+flush per payload.
  fn compress_stream_messages(chunks: &[&[u8]]) -> Vec<Vec<u8>> {
    let mut encoder = Encoder::new(0).expect("encoder");
    let mut frames = Vec::new();
    let mut scratch = vec![0u8; 4096];

    for chunk in chunks {
      let mut out = Vec::new();
      let mut input = InBuffer::around(chunk);
      while input.pos() < chunk.len() {
        let mut output = OutBuffer::around(scratch.as_mut_slice());
        encoder.run(&mut input, &mut output).expect("compress");
        out.extend_from_slice(output.as_slice());
      }
      loop {
        let mut output = OutBuffer::around(scratch.as_mut_slice());
        let remaining = encoder.flush(&mut output).expect("flush");
        out.extend_from_slice(output.as_slice());
        if remaining == 0 {
          break;
        }
      }
      frames.push(out);
    }

    frames
  }

  #[test]
  fn decompresses_multi_message_stream() {
    let a = br#"{"op":10,"d":{"heartbeat_interval":41250}}"#;
    let b = br#"{"op":11}"#;
    let frames = compress_stream_messages(&[a, b]);
    assert_eq!(frames.len(), 2);
    assert!(!frames[0].is_empty());
    assert!(!frames[1].is_empty());

    let mut decomp = TransportDecompress::new().unwrap();
    let out_a = decomp.push(&frames[0]).unwrap().to_vec();
    assert_eq!(out_a, a);
    let out_b = decomp.push(&frames[1]).unwrap().to_vec();
    assert_eq!(out_b, b);
  }

  #[test]
  fn reclaim_shrinks_large_capacity() {
    let mut d = TransportDecompress::new().unwrap();
    d.out.resize(2 * 1024 * 1024, 0);
    let before = d.out.capacity();
    assert!(before > SHRINK_THRESHOLD);
    d.reclaim();
    assert!(d.out.is_empty());
    assert!(d.out.capacity() <= SHRINK_THRESHOLD);
    assert!(d.out.capacity() < before);
    assert!(d.out.capacity() >= KEEP_CAP);
  }

  #[test]
  fn reclaim_noop_when_capacity_small() {
    let mut d = TransportDecompress::new().unwrap();
    let before = d.out.capacity();
    assert!(before <= SHRINK_THRESHOLD);
    d.reclaim();
    assert_eq!(d.out.capacity(), before);
  }
}
