//! Encrypting half of the STREAM construction.

use std::io::{self, Write};

use aead_stream::StreamPrimitive;
use zeroize::Zeroizing;

use super::{NoncePrefix, SEGMENT_LEN, Stream, new_stream, segment_buf};
use crate::{ArchiveKey, CryptoError, Result};

/// Encrypting writer: buffers plaintext into [`SEGMENT_LEN`] segments and
/// writes `ciphertext || tag` per segment. [`EncryptWriter::finish`] MUST be
/// called to emit the final (last-flagged) segment; dropping without it
/// leaves a stream readers reject as truncated.
///
/// A full segment is only emitted once more plaintext arrives, so the final
/// segment is never empty unless the whole stream is. [`Write::flush`]
/// flushes the inner writer but cannot emit a partial segment. After any
/// error the writer is poisoned and every later call fails.
pub struct EncryptWriter<W: Write> {
    pub(super) inner: W,
    stream: Option<Stream>,
    aad: Vec<u8>,
    position: u32,
    buf: Zeroizing<Vec<u8>>,
}

impl<W: Write> std::fmt::Debug for EncryptWriter<W> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EncryptWriter")
            .field("segment", &self.position)
            .field("buffered", &self.buf.len())
            .field("poisoned", &self.stream.is_none())
            .finish_non_exhaustive()
    }
}

impl<W: Write> EncryptWriter<W> {
    /// Start a stream. `aad` (the serialized header) authenticates every segment.
    pub fn new(inner: W, key: &ArchiveKey, nonce: &NoncePrefix, aad: &[u8]) -> Result<Self> {
        Ok(Self {
            inner,
            stream: Some(new_stream(key, nonce)?),
            aad: aad.to_vec(),
            position: 0,
            buf: segment_buf(),
        })
    }

    /// Encrypt the buffered plaintext as segment `position` and write it.
    fn emit(&mut self, last: bool) -> Result<()> {
        let Some(stream) = self.stream.take() else {
            return Err(CryptoError::Io(io::Error::other("encrypt stream poisoned")));
        };
        if !last && self.position == u32::MAX {
            return Err(CryptoError::TooLong);
        }
        stream
            .encrypt_in_place(self.position, last, &self.aad, &mut *self.buf)
            .map_err(|_| CryptoError::TooLong)?;
        self.inner.write_all(&self.buf)?;
        self.buf.clear();
        if !last {
            self.position += 1;
            self.stream = Some(stream);
        }
        Ok(())
    }

    /// Emit the final segment, flush, and return the inner writer.
    pub fn finish(mut self) -> Result<W> {
        self.emit(true)?;
        self.inner.flush()?;
        Ok(self.inner)
    }
}

impl<W: Write> Write for EncryptWriter<W> {
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        if self.stream.is_none() {
            return Err(io::Error::other("encrypt stream poisoned"));
        }
        if data.is_empty() {
            return Ok(0);
        }
        if self.buf.len() == SEGMENT_LEN {
            self.emit(false).map_err(CryptoError::into_io)?;
        }
        let n = data.len().min(SEGMENT_LEN - self.buf.len());
        self.buf.extend_from_slice(&data[..n]);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
