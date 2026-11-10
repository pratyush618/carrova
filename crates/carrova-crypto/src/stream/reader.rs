//! Decrypting half of the STREAM construction.

use std::io::{self, Read};

use aead_stream::StreamPrimitive;
use zeroize::Zeroizing;

use super::{FULL_SEGMENT, NoncePrefix, Stream, TAG_LEN, new_stream, segment_buf};
use crate::{ArchiveKey, CryptoError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ReadState {
    Reading,
    Done,
    /// Poisoned; `true` if the failure was truncation.
    Failed(bool),
}

/// Decrypting reader. Yields plaintext only after each segment
/// authenticates; returns an error (never partial unauthenticated data) on
/// tamper, reorder, or truncation (EOF before the last-flagged segment).
///
/// Segments are released one at a time as they authenticate, so a caller
/// may consume earlier segments before a later one fails; the stream as a
/// whole is only proven complete once `read` returns `Ok(0)`.
/// Errors are `io::Error::other(CryptoError)`: [`CryptoError::Decrypt`]
/// (wrong key, tamper, reorder, trailing bytes, or a cut inside a segment)
/// or [`CryptoError::Truncated`] (stream ends before any final segment, or
/// at a clean segment boundary). After an error every later read fails.
pub struct DecryptReader<R: Read> {
    inner: R,
    stream: Stream,
    aad: Vec<u8>,
    position: u32,
    /// Current segment: ciphertext while filling, plaintext once verified.
    buf: Zeroizing<Vec<u8>>,
    /// Read offset into verified plaintext in `buf`.
    pos: usize,
    /// One byte read past a full segment to learn whether more follows.
    lookahead: Option<u8>,
    state: ReadState,
}

impl<R: Read> std::fmt::Debug for DecryptReader<R> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DecryptReader")
            .field("segment", &self.position)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

impl<R: Read> DecryptReader<R> {
    /// Start reading a stream written by [`EncryptWriter`](super::EncryptWriter)
    /// with the same `aad`.
    pub fn new(inner: R, key: &ArchiveKey, nonce: &NoncePrefix, aad: &[u8]) -> Result<Self> {
        Ok(Self {
            inner,
            stream: new_stream(key, nonce)?,
            aad: aad.to_vec(),
            position: 0,
            buf: segment_buf(),
            pos: 0,
            lookahead: None,
            state: ReadState::Reading,
        })
    }

    /// Read into `buf` until it holds `target` bytes or the inner reader hits EOF.
    fn fill(&mut self, target: usize) -> io::Result<()> {
        let mut chunk = [0u8; 8 * 1024];
        while self.buf.len() < target {
            let want = (target - self.buf.len()).min(chunk.len());
            match self.inner.read(&mut chunk[..want]) {
                Ok(0) => break,
                Ok(n) => self.buf.extend_from_slice(&chunk[..n]),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// Read one byte past the current segment; `None` at EOF.
    fn peek_byte(&mut self) -> io::Result<Option<u8>> {
        let mut b = [0u8; 1];
        loop {
            match self.inner.read(&mut b) {
                Ok(0) => return Ok(None),
                Ok(_) => return Ok(Some(b[0])),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// Load, authenticate and decrypt the next segment into `buf`.
    fn next_segment(&mut self) -> Result<()> {
        self.buf.clear();
        self.pos = 0;
        if let Some(b) = self.lookahead.take() {
            self.buf.push(b);
        }
        self.fill(FULL_SEGMENT)?;
        let more = if self.buf.len() == FULL_SEGMENT {
            self.peek_byte()?
        } else {
            None
        };
        if self.buf.len() < TAG_LEN {
            // No room for even an empty final segment: cut short.
            self.buf.clear();
            return Err(CryptoError::Truncated);
        }
        match more {
            Some(b) => {
                if self.position == u32::MAX {
                    return Err(CryptoError::TooLong);
                }
                self.stream
                    .decrypt_in_place(self.position, false, &self.aad, &mut *self.buf)
                    .map_err(|_| CryptoError::Decrypt)?;
                self.position += 1;
                self.lookahead = Some(b);
            }
            None => {
                let last =
                    self.stream
                        .decrypt_in_place(self.position, true, &self.aad, &mut *self.buf);
                if last.is_err() {
                    // A full segment that authenticates as non-final means
                    // the stream was cut at a segment boundary. (Failed
                    // decryption leaves the buffer untouched.)
                    let truncated = self.buf.len() == FULL_SEGMENT
                        && self
                            .stream
                            .decrypt_in_place(self.position, false, &self.aad, &mut *self.buf)
                            .is_ok();
                    self.buf.clear();
                    return Err(if truncated {
                        CryptoError::Truncated
                    } else {
                        CryptoError::Decrypt
                    });
                }
                self.state = ReadState::Done;
            }
        }
        Ok(())
    }
}

impl<R: Read> Read for DecryptReader<R> {
    fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
        loop {
            if let ReadState::Failed(truncated) = self.state {
                let e = if truncated {
                    CryptoError::Truncated
                } else {
                    CryptoError::Decrypt
                };
                return Err(io::Error::other(e));
            }
            if self.pos < self.buf.len() {
                let n = out.len().min(self.buf.len() - self.pos);
                out[..n].copy_from_slice(&self.buf[self.pos..self.pos + n]);
                self.pos += n;
                return Ok(n);
            }
            if self.state == ReadState::Done || out.is_empty() {
                return Ok(0);
            }
            if let Err(e) = self.next_segment() {
                self.state = ReadState::Failed(matches!(e, CryptoError::Truncated));
                self.buf.clear();
                self.pos = 0;
                return Err(e.into_io());
            }
        }
    }
}
