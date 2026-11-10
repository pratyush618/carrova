//! STREAM construction over XChaCha20-Poly1305: shared segment layout,
//! nonce prefix and stream setup for [`EncryptWriter`] / [`DecryptReader`].

use aead_stream::StreamBE32;
use chacha20poly1305::{KeyInit, XChaCha20Poly1305};
use zeroize::Zeroizing;

use crate::{ArchiveKey, CryptoError, Result};

mod reader;
mod writer;

pub use reader::DecryptReader;
pub use writer::EncryptWriter;

/// Plaintext bytes per STREAM segment (64 KiB).
pub const SEGMENT_LEN: usize = 64 * 1024;
/// STREAM nonce prefix length for XChaCha20-Poly1305 (24 − 4 counter − 1 flag).
pub const NONCE_PREFIX_LEN: usize = 19;
/// Poly1305 tag bytes appended to every segment.
pub const TAG_LEN: usize = 16;

/// Ciphertext segment size for every non-final segment.
const FULL_SEGMENT: usize = SEGMENT_LEN + TAG_LEN;

/// Random per-archive STREAM nonce prefix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoncePrefix(pub [u8; NONCE_PREFIX_LEN]);

impl NoncePrefix {
    /// Fresh prefix from the OS RNG.
    pub fn random() -> Result<Self> {
        let mut b = [0u8; NONCE_PREFIX_LEN];
        getrandom::fill(&mut b).map_err(|_| CryptoError::Rng)?;
        Ok(Self(b))
    }
}

/// Total ciphertext bytes [`EncryptWriter`] produces for `plaintext_len`
/// plaintext bytes (excluding any caller header).
#[must_use]
pub const fn ciphertext_len(plaintext_len: u64) -> u64 {
    let seg = SEGMENT_LEN as u64;
    let segments = if plaintext_len == 0 {
        1
    } else {
        plaintext_len.div_ceil(seg)
    };
    plaintext_len + segments * TAG_LEN as u64
}

type Stream = StreamBE32<XChaCha20Poly1305>;

fn new_stream(key: &ArchiveKey, nonce: &NoncePrefix) -> Result<Stream> {
    let aead = XChaCha20Poly1305::new_from_slice(key.0.as_slice())
        .map_err(|_| CryptoError::InvalidParams("bad key length"))?;
    let nonce = aead_stream::Nonce::<XChaCha20Poly1305, Stream>::try_from(&nonce.0[..])
        .map_err(|_| CryptoError::InvalidParams("bad nonce length"))?;
    Ok(<Stream as aead_stream::NewStream<_>>::from_aead(
        aead, &nonce,
    ))
}

fn segment_buf() -> Zeroizing<Vec<u8>> {
    // Sized once so in-place encryption (which appends the tag) never
    // reallocates and leaves plaintext copies behind.
    Zeroizing::new(Vec::with_capacity(FULL_SEGMENT + 1))
}

// Tests use small `KdfParams` that pass `validate` (the read side has no floor).
#[cfg(test)]
#[path = "../../tests/stream/unit.rs"]
mod tests;
