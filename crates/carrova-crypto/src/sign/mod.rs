//! Ed25519 signatures with a two-level key chain, used to sign recipe
//! bundles.
//!
//! Scheme (user decision 2026-11-08, see `tasks/todo.md` "Phase 4
//! decisions"):
//!
//! 1. An offline **root** key ([`SigningKey`]) certifies a short-lived
//!    **bundle-signer** key: it signs the canonical bytes of a [`KeyCert`]
//!    (key, [`KeyRole`], validity window) under
//!    [`SignContext::KeyCert`], giving a [`SignedKeyCert`].
//! 2. CI holds only the signer key and signs each bundle index under
//!    [`SignContext::BundleIndex`].
//! 3. Clients embed [`TrustRoots`] (a non-empty list of root public keys;
//!    rotation = editing the list) and call [`verify_chain`] with the
//!    current time supplied by the caller (no clock in this crate).
//!
//! # Domain separation
//!
//! Every signed message is `tag ‖ payload`, where the tag is fixed per
//! [`SignContext`] ([`SignContext::tag`]) and ends in a NUL byte, so no
//! tag is a prefix of another and a signature made for one purpose never
//! verifies for another. A [`KeyRole`] lists the contexts its key may sign
//! ([`KeyRole::permits`]); a bundle signer can never sign certificates.
//!
//! # Encodings
//!
//! Public keys and signatures are lowercase hex in text and serde
//! (decoding accepts either case). A [`KeyCert`] is signed over a fixed
//! 49-byte big-endian layout ([`KeyCert::canonical_bytes`]), never over a
//! serde rendering whose field order could drift.
//!
//! Verification uses `ed25519-dalek`'s `verify_strict` (rejects
//! non-canonical and small-order points). Secret keys zeroize on drop and
//! print as `SigningKey(***)`.

use std::fmt;

use data_encoding::{HEXLOWER, HEXLOWER_PERMISSIVE};

mod cert;
mod keys;

pub use cert::{
    CERT_CLOCK_SKEW, KeyCert, KeyRole, MAX_CERT_VALIDITY, MAX_CERT_VALIDITY_DAYS, SignedKeyCert,
    TrustRoots, verify_chain,
};
pub use keys::{SECRET_KEY_LEN, Signature, SigningKey, VerifyingKey};

/// Purpose of a signature; its [`tag`](Self::tag) prefixes every signed
/// message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignContext {
    /// A root key certifying another key ([`KeyCert`]).
    KeyCert,
    /// A bundle signer signing the raw bytes of a recipe bundle index.
    BundleIndex,
}

impl SignContext {
    /// Domain-separation tag prefixed to the signed payload.
    #[must_use]
    pub const fn tag(self) -> &'static [u8] {
        match self {
            Self::KeyCert => b"carrova/keycert/v1\0",
            Self::BundleIndex => b"carrova/bundle-index/v1\0",
        }
    }

    /// `tag ‖ payload`: the bytes actually signed.
    fn message(self, payload: &[u8]) -> Vec<u8> {
        let tag = self.tag();
        let mut msg = Vec::with_capacity(tag.len() + payload.len());
        msg.extend_from_slice(tag);
        msg.extend_from_slice(payload);
        msg
    }
}

/// Why a signature or key chain was rejected. Messages never contain
/// secret material.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SignError {
    /// A hex-encoded key or signature is malformed.
    #[error("invalid {0}: expected hex of the right length")]
    InvalidHex(&'static str),
    /// The bytes are not a valid Ed25519 public key.
    #[error("invalid Ed25519 public key")]
    InvalidKey,
    /// A trust-root list must hold at least one key.
    #[error("trust root list is empty")]
    NoTrustRoots,
    /// The certificate is signed by a key that is not a trusted root.
    #[error("certificate signed by an unknown root key {0}")]
    UnknownRoot(VerifyingKey),
    /// The root's signature over the certificate does not verify.
    #[error("certificate signature is invalid")]
    BadCertSignature,
    /// `now` is before the certificate's `not_before`.
    #[error("signer certificate not valid before {not_before} (now {now})")]
    NotYetValid {
        /// Start of validity (unix seconds).
        not_before: u64,
        /// Time checked (unix seconds).
        now: u64,
    },
    /// `now` is after the certificate's `not_after`.
    #[error("signer certificate expired at {not_after} (now {now})")]
    Expired {
        /// End of validity (unix seconds).
        not_after: u64,
        /// Time checked (unix seconds).
        now: u64,
    },
    /// The certificate's validity window ends before it starts.
    #[error("certificate validity window is inverted")]
    InvertedValidity,
    /// The certificate's validity window is longer than signers may live.
    #[error("certificate is valid for {secs} s, longer than the {max} s maximum")]
    ValidityTooLong {
        /// Window length (`not_after - not_before`, seconds).
        secs: u64,
        /// [`MAX_CERT_VALIDITY`].
        max: u64,
    },
    /// The certified key's role may not sign in this context.
    #[error("a {role} key may not sign {context:?} messages")]
    WrongRole {
        /// Role of the certified key.
        role: KeyRole,
        /// Context the signature claims.
        context: SignContext,
    },
    /// The signature over the message does not verify.
    #[error("signature is invalid")]
    BadSignature,
    /// OS random number generator failed.
    #[error("random number generator failed")]
    Rng,
}

/// Result alias for signing.
pub type Result<T, E = SignError> = std::result::Result<T, E>;

/// Lowercase hex of `bytes`.
fn hex_encode(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(&HEXLOWER.encode_display(bytes), f)
}

/// Decode exactly `N` bytes of hex (either case).
fn hex_decode<const N: usize>(s: &str, what: &'static str) -> Result<[u8; N]> {
    let mut out = [0u8; N];
    if s.len() != 2 * N {
        return Err(SignError::InvalidHex(what));
    }
    HEXLOWER_PERMISSIVE
        .decode_mut(s.as_bytes(), &mut out)
        .map_err(|_| SignError::InvalidHex(what))?;
    Ok(out)
}
