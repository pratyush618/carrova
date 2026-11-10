//! Binary chunk record headers.

use carrova_manifest::{Blake3Hash, ChunkHash, MAX_CHUNK_LEN};

use crate::error::{ProtocolViolation, Result, TransportError};

/// Kind byte of a chunk record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordKind {
    /// `len` chunk bytes follow.
    Data,
    /// The sender cannot supply the chunk (`len` is 0, nothing follows).
    Unavailable,
}

impl RecordKind {
    const fn to_byte(self) -> u8 {
        match self {
            Self::Data => 0,
            Self::Unavailable => 1,
        }
    }

    fn from_byte(b: u8) -> Result<Self> {
        match b {
            0 => Ok(Self::Data),
            1 => Ok(Self::Unavailable),
            _ => Err(ProtocolViolation::UnknownRecordKind.into()),
        }
    }
}

/// A chunk length from the manifest, checked against
/// [`MAX_CHUNK_LEN`] before anything is allocated for it: a longer chunk
/// is a protocol violation (no honest manifest lists one).
pub(crate) fn chunk_len(len: u32) -> Result<usize> {
    if len > MAX_CHUNK_LEN {
        return Err(ProtocolViolation::ChunkTooLong { len }.into());
    }
    usize::try_from(len).map_err(|_| TransportError::from(ProtocolViolation::ChunkLengthOverflow))
}

/// Bytes of a chunk record header.
pub(crate) const RECORD_HEADER_LEN: usize = 1 + 4 + 32;

/// Decoded chunk record header.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordHeader {
    pub(crate) kind: RecordKind,
    pub(crate) len: u32,
    pub(crate) hash: ChunkHash,
}

impl RecordHeader {
    /// Append the header to `out`.
    pub(crate) fn write(&self, out: &mut Vec<u8>) {
        out.push(self.kind.to_byte());
        out.extend_from_slice(&self.len.to_be_bytes());
        out.extend_from_slice(self.hash.as_bytes());
    }

    pub(super) fn parse(b: &[u8; RECORD_HEADER_LEN]) -> Result<Self> {
        let (kind, rest) = b.split_at(1);
        let (len, hash) = rest.split_at(4);
        let mut len_bytes = [0u8; 4];
        len_bytes.copy_from_slice(len);
        let mut hash_bytes = [0u8; 32];
        hash_bytes.copy_from_slice(hash);
        Ok(Self {
            kind: RecordKind::from_byte(kind[0])?,
            len: u32::from_be_bytes(len_bytes),
            hash: Blake3Hash(hash_bytes),
        })
    }
}

#[cfg(test)]
#[path = "../../tests/wire/record/unit.rs"]
mod tests;
