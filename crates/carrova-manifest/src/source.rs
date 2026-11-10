//! Pull-by-hash access to chunk bytes on the sending side.
//!
//! One trait for every sink that consumes a manifest's chunks: the archive
//! writer (Phase 2) and the LAN chunk server (Phase 3). The sender's
//! capture implements it by re-reading captured files.

use carrova_platform::PlatformError;

use crate::chunk::ChunkRef;
use crate::hash::ChunkHash;

/// Supplies chunk bytes by reference. Implementations return exactly the
/// bytes of `chunk`; consumers re-check length and BLAKE3 hash anyway.
pub trait ChunkSource: Send {
    /// Bytes of `chunk` (must be `chunk.len` bytes hashing to `chunk.hash`).
    fn read_chunk(&mut self, chunk: &ChunkRef) -> Result<Vec<u8>, ChunkSourceError>;
}

/// Why a [`ChunkSource`] could not supply a chunk. Messages carry chunk
/// hashes and display paths only, never chunk contents.
#[derive(Debug, thiserror::Error)]
pub enum ChunkSourceError {
    /// The source does not know this chunk (not part of its manifest).
    #[error("chunk {0} is not available from this source")]
    Missing(ChunkHash),
    /// The data behind the chunk changed since it was captured. Holds a
    /// display label of the location (e.g. `user_config:git/config`).
    #[error("{0} changed after capture")]
    Changed(String),
    /// Platform-layer failure opening the chunk's backing file.
    #[error(transparent)]
    Platform(#[from] PlatformError),
    /// Plain I/O failure reading the chunk.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The consumer's user cancelled; nothing is wrong with the chunk.
    #[error("cancelled")]
    Cancelled,
}
