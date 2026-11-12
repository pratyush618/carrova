//! LAN pairing: short codes, SPAKE2, key confirmation and resume secrets.
//!
//! Protocol (signed off 2026-11-08, see `tasks/todo.md` "Phase 3 decisions"):
//!
//! 1. The sender shows a [`PairingCode`] (`<nameplate>-<word>-<word>`); the
//!    user types it on the receiver. The [`Nameplate`] is public (it picks
//!    the sender on the LAN); the words are the secret.
//! 2. Inside a mutually authenticated TLS 1.3 (QUIC) connection with
//!    throwaway certificates, both sides run SPAKE2 over the Ed25519 group
//!    ([`Pake`]; A = sender, B = receiver, identities [`PakeRole::identity`]).
//! 3. The SPAKE2 key is expanded with HKDF-SHA256 into a confirmation key
//!    and a [`ResumeSecret`] ([`SessionKeys`]).
//! 4. Key confirmation: each side sends `HMAC-SHA256(K_confirm, role ‖
//!    binding)` where the [`ChannelBinding`] is the TLS exporter
//!    ([`EXPORTER_LABEL`]) ‖ SHA-256(sender cert) ‖ SHA-256(receiver cert).
//!    The sender confirms first; both checks are constant-time. A wrong code
//!    or a man in the middle (different TLS session or certificates) fails
//!    here with [`PairingError::ConfirmationFailed`].
//! 5. Reconnects skip SPAKE2: the receiver looks up the sender by a
//!    [`ResumeTag`] rotated per advertisement ([`ResumeSecret::tag_for`] a
//!    random [`TagNonce`], so adverts are unlinkable), names the session in
//!    its `Hello` with a tag of another [`TagPurpose`], and both prove
//!    possession of the [`ResumeSecret`] over the fresh connection's
//!    binding ([`ResumeSecret::prove`]).
//! 6. A third HKDF output, the [`KnownSecret`], outlives the session when
//!    both sides remember each other (ADR 0001): it makes per-advert
//!    [`KnownTag`]s that let a known device recognise this one nearby.
//!
//! Every label used for domain separation is a constant in this module.
//! Secrets ([`PairingCode`], [`SessionKeys`], [`ResumeSecret`],
//! [`KnownSecret`], [`ChannelBinding`]) zeroize on drop and redact themselves in `Debug`.
//!
//! # Codes
//!
//! Words come from the EFF short wordlist 2.0 (1296 words, ≈10.3 bits each,
//! unique 3-letter prefixes), so a code carries ≈20.7 secret bits; SPAKE2
//! gives an attacker one online guess per code. Parsing accepts any
//! unambiguous prefix of at least [`MIN_WORD_PREFIX`] letters. The SPAKE2
//! password is the canonical encoding of (nameplate, word indices), so
//! every accepted spelling of a code yields the same password.
//!
//! Accepted risk: `spake2` 0.4 draws its scalar from `OsRng` (panics if the
//! OS RNG fails; `Pake::start` is infallible by contract) and does not
//! zeroize its copy of the password or its scalars.
//!
//! # Modules
//!
//! [`PairingCode`] and [`Nameplate`] live in `code`, [`CertHash`] and
//! [`ChannelBinding`] in `binding`, the SPAKE2 exchange and its
//! [`SessionKeys`] in `pake`, the [`ResumeSecret`] with its tags in
//! `keys`, and the [`KnownSecret`] with its tags in `known`. Every protocol label stays in this file.

use std::fmt;
use std::ops::RangeInclusive;

use data_encoding::{HEXLOWER, HEXLOWER_PERMISSIVE};
use hmac::Hmac;
use sha2::Sha256;

mod binding;
mod code;
mod keys;
mod known;
mod pake;
#[cfg(test)]
#[path = "../../tests/pairing/unit.rs"]
mod tests;
mod wordlist;

pub use binding::{CertHash, ChannelBinding};
pub use code::{Nameplate, PairingCode};
pub use keys::{ResumeSecret, ResumeTag, TagNonce, TagPurpose};
pub use known::{KnownPurpose, KnownSecret, KnownTag};
pub use pake::{Confirmation, Pake, PakeMessage, PakeRole, SessionKeys};

/// Valid nameplates (shown before the words of a code).
pub const NAMEPLATES: RangeInclusive<u16> = 1..=999;
/// TLS exporter label of the channel binding (RFC 8446 §7.5).
pub const EXPORTER_LABEL: &[u8] = b"EXPORTER-carrova/1/pair";
/// TLS exporter output length used in the channel binding.
pub const EXPORTER_LEN: usize = 32;
/// Length of a SHA-256 certificate hash.
pub const CERT_HASH_LEN: usize = 32;
/// Length of a serialized [`ChannelBinding`].
pub const BINDING_LEN: usize = EXPORTER_LEN + 2 * CERT_HASH_LEN;
/// Length of a [`ResumeSecret`].
pub const RESUME_SECRET_LEN: usize = 32;
/// Length of a [`ResumeTag`] (truncated HMAC, advertised in discovery).
pub const RESUME_TAG_LEN: usize = 16;
/// Length of a [`KnownSecret`].
pub const KNOWN_SECRET_LEN: usize = 32;
/// Length of a [`KnownTag`] (truncated HMAC; up to 13 fit one TXT string).
pub const KNOWN_TAG_LEN: usize = 8;
/// Length of a [`TagNonce`].
pub const TAG_NONCE_LEN: usize = 16;
/// Length of a [`Confirmation`] (HMAC-SHA256 output).
pub const CONFIRMATION_LEN: usize = 32;
/// Number of secret words in a [`PairingCode`].
pub const CODE_WORDS: usize = 2;
/// Shortest word prefix accepted when parsing a code (the word list has
/// unique prefixes of this length).
pub const MIN_WORD_PREFIX: usize = 3;
/// Separator between the parts of a displayed code.
pub const CODE_SEPARATOR: char = '-';

/// SPAKE2 identity of the sender (side A).
const SENDER_ID: &[u8] = b"carrova/1/sender";
/// SPAKE2 identity of the receiver (side B).
const RECEIVER_ID: &[u8] = b"carrova/1/receiver";
/// HKDF salt for the SPAKE2 output.
const HKDF_SALT: &[u8] = b"carrova/1/pake";
/// HKDF info of the key-confirmation key.
const INFO_CONFIRM: &[u8] = b"carrova/1/confirm";
/// HKDF info of the resume secret.
const INFO_RESUME: &[u8] = b"carrova/1/resume";
/// HKDF info of the known-device secret.
const INFO_KNOWN: &[u8] = b"carrova/1/known";
/// HMAC label prefixed to the nonce of a discovery [`KnownTag`].
const KNOWN_TAG_DISCOVERY: &[u8] = b"carrova/1/known-discovery";
/// HMAC label prefixed to the nonce of a discovery [`ResumeTag`].
const RESUME_TAG_DISCOVERY: &[u8] = b"carrova/1/resume-tag";
/// HMAC label prefixed to the nonce of a resume-`Hello` [`ResumeTag`].
const RESUME_TAG_HELLO: &[u8] = b"carrova/1/resume-hello";
/// Domain label prefixed to the SPAKE2 password encoding.
const PASSWORD_LABEL: &[u8] = b"carrova/1/code";
/// SPAKE2 password: label ‖ BE16 nameplate ‖ BE16 word index per word.
const PASSWORD_LEN: usize = PASSWORD_LABEL.len() + 2 * (1 + CODE_WORDS);

type HmacSha256 = Hmac<Sha256>;

/// Errors from pairing. Messages never contain codes or key material.
#[derive(Debug, thiserror::Error)]
pub enum PairingError {
    /// A typed pairing code (or its nameplate) is malformed.
    #[error("invalid pairing code: {0}")]
    InvalidCode(CodeError),
    /// A hex-encoded value (resume tag, certificate hash) is malformed.
    #[error("invalid {0}: expected lowercase or uppercase hex of the right length")]
    InvalidHex(&'static str),
    /// The peer's SPAKE2 message is malformed or from the wrong side.
    #[error("invalid PAKE message from peer")]
    InvalidPakeMessage,
    /// HKDF expansion failed (cannot happen for the fixed output sizes used).
    #[error("key schedule failed")]
    KeySchedule,
    /// Key confirmation failed: wrong code, or the channel is not the one
    /// the peer sees (man in the middle).
    #[error("pairing failed: wrong code or tampered connection")]
    ConfirmationFailed,
    /// OS random number generator failed.
    #[error("random number generator failed")]
    Rng,
}

/// Why a typed pairing code was rejected. Never carries the typed words.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CodeError {
    /// Not `<nameplate>` followed by exactly [`CODE_WORDS`] words.
    #[error("expected <number>-<word>-<word>")]
    Shape,
    /// The nameplate is not a number in [`NAMEPLATES`].
    #[error("the number must be from 1 to 999")]
    Nameplate,
    /// Word number `position` (1-based) is not in the word list (or is a
    /// prefix shorter than [`MIN_WORD_PREFIX`] letters).
    #[error("word {position} is not in the word list")]
    UnknownWord {
        /// 1-based position of the word in the code.
        position: usize,
    },
}

impl From<CodeError> for PairingError {
    fn from(e: CodeError) -> Self {
        Self::InvalidCode(e)
    }
}

/// Result alias for pairing.
pub type Result<T, E = PairingError> = std::result::Result<T, E>;

// ------------------------------------------------------------------ hex

/// Lowercase hex of `bytes`.
fn hex_encode(bytes: &[u8], f: &mut fmt::Formatter<'_>) -> fmt::Result {
    fmt::Display::fmt(&HEXLOWER.encode_display(bytes), f)
}

/// Decode exactly `N` bytes of hex (either case).
fn hex_decode<const N: usize>(s: &str, what: &'static str) -> Result<[u8; N]> {
    let mut out = [0u8; N];
    if s.len() != 2 * N {
        return Err(PairingError::InvalidHex(what));
    }
    HEXLOWER_PERMISSIVE
        .decode_mut(s.as_bytes(), &mut out)
        .map_err(|_| PairingError::InvalidHex(what))?;
    Ok(out)
}
