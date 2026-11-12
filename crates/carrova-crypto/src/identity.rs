//! Device identity (ADR 0001 §1–3): one Ed25519 key per install, proven
//! inside a code-paired connection.
//!
//! A device *is* its full 32-byte public key ([`DeviceId`]); every lookup,
//! store key and trust decision uses it. The [`Fingerprint`] (first 4
//! bytes of its SHA-256) is a display label only: 32 bits prove nothing.
//!
//! Each side signs, under [`SignContext::DeviceIdentity`],
//! `role identity ‖ ChannelBinding ‖ own public key`: the role stops
//! reflection, the binding ties the proof to one TLS session. The key
//! seed zeroizes on drop; no `Debug` output here shows key bytes (public
//! keys print as their fingerprint).

use std::fmt;
use std::str::FromStr;

use data_encoding::{HEXLOWER, HEXUPPER_PERMISSIVE};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::pairing::{ChannelBinding, PakeRole};
use crate::sign::{SECRET_KEY_LEN, SignContext, Signature, SigningKey, VerifyingKey};

/// Length of a [`DeviceKey`] seed.
pub const DEVICE_SEED_LEN: usize = SECRET_KEY_LEN;
/// Length of a [`DevicePublic`] key.
pub const DEVICE_PUBLIC_LEN: usize = ed25519_dalek::PUBLIC_KEY_LENGTH;
/// Length of an identity signature.
pub const IDENTITY_SIG_LEN: usize = ed25519_dalek::SIGNATURE_LENGTH;
/// Bytes of SHA-256 shown in a [`Fingerprint`].
const FINGERPRINT_LEN: usize = 4;
/// Shown between the two halves of a [`Fingerprint`] (U+00B7).
const FINGERPRINT_SEPARATOR: char = '\u{b7}';

/// Why a device key or identity proof was rejected. Never carries key bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum IdentityError {
    /// A public key or fingerprint is not hex of the right length.
    #[error("invalid {0}: expected hex of the right length")]
    InvalidHex(&'static str),
    /// The bytes are not a valid Ed25519 public key.
    #[error("invalid device public key")]
    InvalidKey,
    /// The peer claims this device's own key.
    #[error("peer claims this device's own identity")]
    OwnKey,
    /// The identity signature does not verify.
    #[error("device identity signature is invalid")]
    BadSignature,
    /// OS random number generator failed.
    #[error("random number generator failed")]
    Rng,
}

/// This install's Ed25519 identity key. Zeroized on drop; `Debug` is
/// redacted.
pub struct DeviceKey(SigningKey);

impl DeviceKey {
    /// Fresh key from the OS RNG.
    pub fn generate() -> Result<Self, IdentityError> {
        SigningKey::generate()
            .map(Self)
            .map_err(|_| IdentityError::Rng)
    }

    /// Key from its persisted seed (copied; the caller zeroizes its own).
    #[must_use]
    pub fn from_bytes(seed: &[u8; DEVICE_SEED_LEN]) -> Self {
        Self(SigningKey::from_seed(seed))
    }

    /// The seed, for persisting only.
    #[must_use]
    pub fn expose_seed(&self) -> &[u8; DEVICE_SEED_LEN] {
        self.0.expose_seed()
    }

    /// Public half: this device's identity.
    #[must_use]
    pub fn public(&self) -> DevicePublic {
        DevicePublic(self.0.verifying_key())
    }

    /// This side's identity proof for `binding`, signed as `role`.
    #[must_use]
    pub fn sign_binding(&self, role: PakeRole, binding: &ChannelBinding) -> [u8; IDENTITY_SIG_LEN] {
        let payload = identity_payload(role, binding, &self.public());
        self.0
            .sign(SignContext::DeviceIdentity, &payload)
            .to_bytes()
    }

    /// Check the peer's identity proof: `peer` signed `binding` as
    /// `peer_role` (strict Ed25519). A peer claiming this device's own key
    /// is refused, so a reflected proof never passes.
    pub fn verify_peer(
        &self,
        peer: &DevicePublic,
        peer_role: PakeRole,
        binding: &ChannelBinding,
        sig: &[u8; IDENTITY_SIG_LEN],
    ) -> Result<(), IdentityError> {
        if *peer == self.public() {
            return Err(IdentityError::OwnKey);
        }
        let payload = identity_payload(peer_role, binding, peer);
        peer.0
            .verify(
                SignContext::DeviceIdentity,
                &payload,
                &Signature::from_bytes(sig),
            )
            .map_err(|_| IdentityError::BadSignature)
    }
}

impl fmt::Debug for DeviceKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DeviceKey(<redacted>)")
    }
}

/// `role identity ‖ binding ‖ public key` (ADR 0001 §3).
fn identity_payload(
    role: PakeRole,
    binding: &ChannelBinding,
    public: &DevicePublic,
) -> Zeroizing<Vec<u8>> {
    let mut payload = Zeroizing::new(Vec::new());
    payload.extend_from_slice(role.identity());
    payload.extend_from_slice(binding.as_bytes());
    payload.extend_from_slice(public.as_bytes());
    payload
}

/// A device's public identity key. Serde form: 64 lowercase hex digits.
/// There is deliberately no `Display` (full keys are never logged);
/// `Debug` shows the [`Fingerprint`].
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct DevicePublic(VerifyingKey);

/// The identity of a device: its full public key.
pub type DeviceId = DevicePublic;

impl DevicePublic {
    /// Key from its 32-byte encoding.
    pub fn from_bytes(bytes: &[u8; DEVICE_PUBLIC_LEN]) -> Result<Self, IdentityError> {
        VerifyingKey::from_bytes(bytes)
            .map(Self)
            .map_err(|_| IdentityError::InvalidKey)
    }

    /// 32-byte encoding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; DEVICE_PUBLIC_LEN] {
        self.0.as_bytes()
    }

    /// Lowercase hex (file names and storage only, never logs).
    #[must_use]
    pub fn to_hex(&self) -> String {
        HEXLOWER.encode(self.as_bytes())
    }
}

impl fmt::Debug for DevicePublic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "DevicePublic({})", Fingerprint::of(self))
    }
}

impl FromStr for DevicePublic {
    type Err = IdentityError;

    /// 64 hex digits, either case.
    fn from_str(s: &str) -> Result<Self, IdentityError> {
        Self::from_bytes(&decode_hex(s, "device public key")?)
    }
}

impl serde::Serialize for DevicePublic {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_hex())
    }
}

impl<'de> serde::Deserialize<'de> for DevicePublic {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        String::deserialize(d)?
            .parse()
            .map_err(serde::de::Error::custom)
    }
}

/// Display label of a device: first 4 bytes of SHA-256(public key),
/// upper-case hex, shown `7F3A·19C2` (also in `Debug`). Not a security
/// check.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Fingerprint([u8; FINGERPRINT_LEN]);

impl Fingerprint {
    /// Fingerprint of `public`.
    #[must_use]
    pub fn of(public: &DevicePublic) -> Self {
        let digest = Sha256::digest(public.as_bytes());
        let mut fp = [0u8; FINGERPRINT_LEN];
        fp.copy_from_slice(&digest[..FINGERPRINT_LEN]);
        Self(fp)
    }
}

impl fmt::Display for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hex = data_encoding::HEXUPPER.encode(&self.0);
        let (a, b) = hex.split_at(hex.len() / 2);
        write!(f, "{a}{FINGERPRINT_SEPARATOR}{b}")
    }
}

impl fmt::Debug for Fingerprint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Fingerprint({self})")
    }
}

impl FromStr for Fingerprint {
    type Err = IdentityError;

    /// 8 hex digits, either case, optionally split by `·` or `-` (easy to
    /// type).
    fn from_str(s: &str) -> Result<Self, IdentityError> {
        let plain: String = s
            .trim()
            .chars()
            .filter(|&c| c != FINGERPRINT_SEPARATOR && c != '-')
            .collect();
        decode_hex(&plain, "fingerprint").map(Self)
    }
}

/// Decode exactly `N` bytes of hex (either case).
fn decode_hex<const N: usize>(s: &str, what: &'static str) -> Result<[u8; N], IdentityError> {
    let s = s.trim();
    let mut out = [0u8; N];
    if s.len() != 2 * N {
        return Err(IdentityError::InvalidHex(what));
    }
    HEXUPPER_PERMISSIVE
        .decode_mut(s.as_bytes(), &mut out)
        .map_err(|_| IdentityError::InvalidHex(what))?;
    Ok(out)
}

#[cfg(test)]
#[path = "../tests/identity/unit.rs"]
mod tests;
