//! Buffered, cancel-safe reader for frames and chunk records.

use quinn::{ReadError, ReadExactError, RecvStream};
use serde::de::DeserializeOwned;

use super::frame::{FRAME_HEADER_LEN, decode_body};
use super::record::{RECORD_HEADER_LEN, RecordHeader};
use crate::MAX_FRAME_LEN;
use crate::error::{ProtocolViolation, Result, TransportError};

/// Largest single read from a stream into a [`StreamReader`] buffer.
const READ_CHUNK_MAX: usize = 256 * 1024;

/// Buffered reader over a QUIC receive stream. Frame reads are cancel-safe
/// (partial frames stay buffered), so a timed-out `recv` loses nothing.
#[derive(Debug)]
pub(crate) struct StreamReader {
    stream: RecvStream,
    buf: Vec<u8>,
    /// Bytes of `buf` already consumed.
    pos: usize,
    /// The peer reset the stream (a cancelled fetch, not a failure).
    reset_by_peer: bool,
}

impl StreamReader {
    pub(crate) fn new(stream: RecvStream) -> Self {
        Self {
            stream,
            buf: Vec::new(),
            pos: 0,
            reset_by_peer: false,
        }
    }

    /// Whether the last read failed because the peer reset the stream.
    pub(crate) fn reset_by_peer(&self) -> bool {
        self.reset_by_peer
    }

    fn read_failed(&mut self, err: &ReadError) -> TransportError {
        self.reset_by_peer = matches!(err, ReadError::Reset(_));
        TransportError::read(err)
    }

    pub(crate) fn stream_mut(&mut self) -> &mut RecvStream {
        &mut self.stream
    }

    fn buffered(&self) -> &[u8] {
        &self.buf[self.pos..]
    }

    fn consume(&mut self, n: usize) {
        self.pos += n;
        if self.pos == self.buf.len() {
            self.buf.clear();
            self.pos = 0;
        }
    }

    /// Buffer at least `n` bytes. `Ok(false)` if the stream finished with
    /// nothing buffered (a clean end between messages).
    async fn fill(&mut self, n: usize) -> Result<bool> {
        while self.buffered().len() < n {
            if self.pos > 0 {
                self.buf.drain(..self.pos);
                self.pos = 0;
            }
            let chunk = self.stream.read_chunk(READ_CHUNK_MAX, true).await;
            match chunk.map_err(|e| self.read_failed(&e))? {
                Some(chunk) => self.buf.extend_from_slice(&chunk.bytes),
                None if self.buf.is_empty() => return Ok(false),
                None => return Err(ProtocolViolation::TruncatedMessage.into()),
            }
        }
        Ok(true)
    }

    /// Next control frame, `None` at a clean end of stream.
    pub(crate) async fn frame<T: DeserializeOwned>(&mut self) -> Result<Option<T>> {
        if !self.fill(FRAME_HEADER_LEN).await? {
            return Ok(None);
        }
        let mut prefix = [0u8; FRAME_HEADER_LEN];
        prefix.copy_from_slice(&self.buffered()[..FRAME_HEADER_LEN]);
        let len = usize::try_from(u32::from_be_bytes(prefix))
            .map_err(|_| TransportError::FrameTooLarge { len: usize::MAX })?;
        if len > MAX_FRAME_LEN {
            return Err(TransportError::FrameTooLarge { len });
        }
        if !self.fill(FRAME_HEADER_LEN + len).await? {
            return Err(ProtocolViolation::TruncatedMessage.into());
        }
        let msg = decode_body(&self.buffered()[FRAME_HEADER_LEN..FRAME_HEADER_LEN + len]);
        self.consume(FRAME_HEADER_LEN + len);
        msg.map(Some)
    }

    /// Next control frame; end of stream is an error (peer went away).
    pub(crate) async fn expect_frame<T: DeserializeOwned>(&mut self) -> Result<T> {
        self.frame().await?.ok_or(TransportError::PeerLost)
    }

    /// Next chunk record header, `None` at a clean end of stream.
    pub(crate) async fn record_header(&mut self) -> Result<Option<RecordHeader>> {
        if !self.fill(RECORD_HEADER_LEN).await? {
            return Ok(None);
        }
        let mut raw = [0u8; RECORD_HEADER_LEN];
        raw.copy_from_slice(&self.buffered()[..RECORD_HEADER_LEN]);
        self.consume(RECORD_HEADER_LEN);
        RecordHeader::parse(&raw).map(Some)
    }

    /// Exactly `len` payload bytes: what is buffered, then straight from
    /// the stream into the output (no second copy for large chunks).
    pub(crate) async fn payload(&mut self, len: usize) -> Result<Vec<u8>> {
        let mut out = vec![0u8; len];
        let have = self.buffered().len().min(len);
        out[..have].copy_from_slice(&self.buffered()[..have]);
        self.consume(have);
        if have < len {
            match self.stream.read_exact(&mut out[have..]).await {
                Ok(()) => {}
                Err(ReadExactError::ReadError(e)) => return Err(self.read_failed(&e)),
                Err(e) => return Err(TransportError::read_exact(&e)),
            }
        }
        Ok(out)
    }
}
