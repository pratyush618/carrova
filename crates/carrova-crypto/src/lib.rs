//! Carrova cryptography: passphrase key derivation and streaming
//! authenticated encryption for archives; LAN pairing (SPAKE2, key
//! confirmation, resume secrets) in [`pairing`]; Ed25519 signatures with
//! a root → signer certificate chain (recipe bundles) in [`sign`].
//!
//! Scheme (signed off 2026-11-08): Argon2id → 32-byte key;
//! XChaCha20-Poly1305 in the STREAM construction (BE32 counter, 19-byte
//! nonce prefix, [`SEGMENT_LEN`]-byte plaintext segments). The caller's
//! header bytes are the associated data of **every** segment, so header
//! tampering fails decryption. Audited RustCrypto crates only; no
//! hand-rolled primitives.
//!
//! Secrets ([`Passphrase`], [`ArchiveKey`]) zeroize on drop and never
//! print their contents via `Debug`.
//!
//! # Ciphertext layout
//!
//! An encrypted stream of `n` plaintext bytes is a sequence of
//! `s = max(1, ceil(n / SEGMENT_LEN))` segments, with no framing between them:
//!
//! ```text
//! segment i = XChaCha20(plaintext_i) || Poly1305 tag (16 bytes)
//! nonce_i   = nonce_prefix (19) || BE32(i) (4) || last flag (1: 0x01 on the final segment, else 0x00)
//! aad_i     = caller's header bytes (same for every segment)
//! ```
//!
//! Every non-final segment carries exactly [`SEGMENT_LEN`] plaintext bytes;
//! the final segment carries `0..=SEGMENT_LEN` bytes (it is empty only when
//! the whole stream is empty). Overhead is a constant [`TAG_LEN`] bytes per
//! segment; see [`ciphertext_len`]. The final segment is recognised by EOF
//! and authenticated with the last flag, so truncation at a segment
//! boundary, reordering, and appended bytes all fail authentication.
//!
//! # Errors through `std::io`
//!
//! [`EncryptWriter`] and [`DecryptReader`] implement
//! [`Write`](std::io::Write)/[`Read`](std::io::Read), so
//! their failures surface as [`std::io::Error`]. Crypto failures are wrapped
//! with `std::io::Error::other` around a [`CryptoError`]; recover the typed
//! error with [`CryptoError::from_io`] (also what `From<io::Error>` does, so
//! `?` in a function returning [`Result`] restores it automatically).

/// Serde as a string via `Display` / `FromStr` (hex for keys, tags and
/// hashes). Shared by [`pairing`] and [`sign`].
macro_rules! serde_via_str {
    ($ty:ty) => {
        impl serde::Serialize for $ty {
            fn serialize<S: serde::Serializer>(
                &self,
                s: S,
            ) -> std::result::Result<S::Ok, S::Error> {
                s.collect_str(self)
            }
        }

        impl<'de> serde::Deserialize<'de> for $ty {
            fn deserialize<D: serde::Deserializer<'de>>(
                d: D,
            ) -> std::result::Result<Self, D::Error> {
                let s = String::deserialize(d)?;
                s.parse().map_err(serde::de::Error::custom)
            }
        }
    };
}

pub mod pairing;
pub mod sign;

mod error;
mod kdf;
mod passphrase;
mod stream;

pub use error::{CryptoError, Result};
pub use kdf::{ArchiveKey, KdfParams, SALT_LEN, Salt, derive_key};
pub use passphrase::Passphrase;
pub use stream::{
    DecryptReader, EncryptWriter, NONCE_PREFIX_LEN, NoncePrefix, SEGMENT_LEN, TAG_LEN,
    ciphertext_len,
};
