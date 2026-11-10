//! Crate error type and result alias.

use std::io;

/// Errors from this crate. Messages never contain secret material.
#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    /// KDF parameters outside accepted bounds (protects readers from DoS).
    #[error("invalid key-derivation parameters: {0}")]
    InvalidParams(&'static str),
    /// Key derivation failed.
    #[error("key derivation failed")]
    Kdf,
    /// Authentication failed: wrong passphrase, or data tampered/reordered.
    #[error("decryption failed: wrong passphrase or corrupted data")]
    Decrypt,
    /// The stream ended before its final segment (truncated).
    #[error("encrypted stream truncated")]
    Truncated,
    /// The stream exceeds the STREAM counter range (2^32 segments).
    #[error("encrypted stream too long")]
    TooLong,
    /// OS random number generator failed.
    #[error("random number generator failed")]
    Rng,
    /// Underlying reader/writer failed.
    #[error("I/O error: {0}")]
    Io(std::io::Error),
}

impl CryptoError {
    /// Recover a [`CryptoError`] carried inside an `io::Error` produced by
    /// [`EncryptWriter`](crate::EncryptWriter)/[`DecryptReader`](crate::DecryptReader);
    /// any other I/O error becomes [`CryptoError::Io`].
    #[must_use]
    pub fn from_io(err: io::Error) -> Self {
        if Self::in_io(&err).is_none() {
            return Self::Io(err);
        }
        let Some(inner) = err.into_inner() else {
            return Self::Io(io::Error::other("I/O error")); // unreachable: checked above
        };
        match inner.downcast::<Self>() {
            Ok(c) => *c,
            Err(other) => Self::Io(io::Error::other(other)), // unreachable: checked above
        }
    }

    /// Borrow the [`CryptoError`] carried inside an `io::Error`, if any.
    #[must_use]
    pub fn in_io(err: &io::Error) -> Option<&Self> {
        err.get_ref().and_then(|e| e.downcast_ref::<Self>())
    }

    pub(crate) fn into_io(self) -> io::Error {
        match self {
            Self::Io(e) => e,
            other => io::Error::other(other),
        }
    }
}

impl From<io::Error> for CryptoError {
    fn from(err: io::Error) -> Self {
        Self::from_io(err)
    }
}

/// Crate result alias.
pub type Result<T, E = CryptoError> = std::result::Result<T, E>;

#[cfg(test)]
#[path = "../tests/error/unit.rs"]
mod tests;
