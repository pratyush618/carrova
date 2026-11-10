//! [`ArchiveError`] and how I/O errors map onto it.

use std::io;

use carrova_crypto::CryptoError;
use carrova_manifest::{ChunkHash, ChunkSourceError, ManifestError};
use carrova_platform::PlatformError;

/// Errors from this crate. Never contain secret material.
#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("not a Carrova archive (bad magic)")]
    BadMagic,
    #[error("unsupported archive version {0}")]
    UnsupportedVersion(u16),
    #[error("unsupported archive flags {0:#x}")]
    UnsupportedFlags(u16),
    #[error("malformed archive: {0}")]
    Malformed(String),
    #[error("chunk {0} failed hash verification")]
    ChunkHashMismatch(ChunkHash),
    #[error("archive is missing {0} chunk(s) referenced by its manifest")]
    MissingChunks(usize),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    Manifest(#[from] ManifestError),
    #[error(transparent)]
    Platform(#[from] PlatformError),
    /// The writer's [`ChunkSource`](carrova_manifest::ChunkSource) could not supply a chunk.
    #[error(transparent)]
    Source(#[from] ChunkSourceError),
    /// Plain I/O failure. Crypto failures that travel through `io::Error`
    /// (from the STREAM reader/writer) are unwrapped into
    /// [`ArchiveError::Crypto`] by the `From<io::Error>` impl.
    #[error("I/O error: {0}")]
    Io(std::io::Error),
}

impl From<io::Error> for ArchiveError {
    fn from(err: io::Error) -> Self {
        if CryptoError::in_io(&err).is_some() {
            Self::Crypto(CryptoError::from_io(err))
        } else {
            Self::Io(err)
        }
    }
}

/// Map an error from reading the decrypted body: a clean end of the
/// authenticated stream in the middle of a record is malformed.
pub(crate) fn body_err(err: io::Error) -> ArchiveError {
    if err.kind() == io::ErrorKind::UnexpectedEof && CryptoError::in_io(&err).is_none() {
        ArchiveError::Malformed("unexpected end of archive body".into())
    } else {
        err.into()
    }
}

pub(crate) fn malformed(msg: impl Into<String>) -> ArchiveError {
    ArchiveError::Malformed(msg.into())
}

/// Crate result alias.
pub type Result<T, E = ArchiveError> = std::result::Result<T, E>;
