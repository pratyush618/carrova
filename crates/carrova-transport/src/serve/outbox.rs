//! Coalescing record writer for one chunk stream.

use carrova_manifest::ChunkRef;
use quinn::{SendStream, WriteError};

use super::add;
use crate::connection::ServeEnd;
use crate::error::{Result, TransportError};
use crate::tuning::RESPONSE_COALESCE_BYTES;
use crate::wire::{RECORD_HEADER_LEN, RecordHeader, RecordKind};

/// Why the response side of a chunk stream ended early.
pub(super) enum Respond {
    /// The receiver sent STOP_SENDING (cancelled fetch).
    Stopped,
    Failed(TransportError),
}

/// Records waiting to be written as one coalesced write.
pub(super) struct Outbox {
    pub(super) buf: Vec<u8>,
    /// Chunks and bytes in `buf`.
    queued: ServeEnd,
}

impl Outbox {
    pub(super) fn new() -> Self {
        Self {
            buf: Vec::with_capacity(RESPONSE_COALESCE_BYTES + RECORD_HEADER_LEN),
            queued: ServeEnd::default(),
        }
    }

    pub(super) fn push(&mut self, chunk: &ChunkRef, data: Option<Vec<u8>>) {
        let header = match &data {
            Some(_) => RecordHeader {
                kind: RecordKind::Data,
                len: chunk.len,
                hash: chunk.hash,
            },
            None => RecordHeader {
                kind: RecordKind::Unavailable,
                len: 0,
                hash: chunk.hash,
            },
        };
        header.write(&mut self.buf);
        if let Some(bytes) = data {
            self.buf.extend_from_slice(&bytes);
            self.queued.chunks += 1;
            self.queued.bytes += bytes.len() as u64;
        }
    }

    /// Write everything queued; on success it counts as `sent`.
    pub(super) async fn flush(
        &mut self,
        send: &mut SendStream,
        sent: &mut ServeEnd,
    ) -> Result<(), Respond> {
        if self.buf.is_empty() {
            return Ok(());
        }
        match send.write_all(&self.buf).await {
            Ok(()) => {
                self.buf.clear();
                add(sent, std::mem::take(&mut self.queued));
                Ok(())
            }
            Err(WriteError::Stopped(_)) => Err(Respond::Stopped),
            Err(e) => Err(Respond::Failed(TransportError::write(&e))),
        }
    }
}
