//! Known-device secret and its discovery tags (ADR 0001 §4–5).

use std::fmt;
use std::str::FromStr;

use hmac::Mac;
use zeroize::Zeroizing;

use super::keys::MacKey;
use super::{
    HmacSha256, KNOWN_SECRET_LEN, KNOWN_TAG_DISCOVERY, KNOWN_TAG_LEN, PairingError, Result,
    TagNonce, hex_decode, hex_encode,
};

/// Secret two devices share after a code pairing in which both remembered
/// each other. Derived from the SPAKE2 key like the resume secret, under
/// its own HKDF info; stored per known device.
#[derive(Clone)]
pub struct KnownSecret(Zeroizing<[u8; KNOWN_SECRET_LEN]>);

impl KnownSecret {
    /// Wrap a freshly derived secret without copying it out of `Zeroizing`.
    pub(super) fn from_key(key: Zeroizing<[u8; KNOWN_SECRET_LEN]>) -> Self {
        Self(key)
    }

    /// Restore a persisted secret.
    #[must_use]
    pub fn from_bytes(bytes: [u8; KNOWN_SECRET_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Raw bytes, for persisting only.
    #[must_use]
    pub fn expose_bytes(&self) -> &[u8; KNOWN_SECRET_LEN] {
        &self.0
    }

    /// Tag for one `purpose` and `nonce`:
    /// `HMAC(secret, label(purpose) ‖ nonce)[..8]`. A fresh [`TagNonce`]
    /// per advert keeps tags unlinkable for anyone without the secret.
    pub fn tag_for(&self, purpose: KnownPurpose, nonce: &TagNonce) -> Result<KnownTag> {
        let full = self.tag_mac(purpose, nonce)?.finalize().into_bytes();
        let mut tag = [0u8; KNOWN_TAG_LEN];
        tag.copy_from_slice(&full[..KNOWN_TAG_LEN]);
        Ok(KnownTag(tag))
    }

    /// Whether `tag` is this secret's `purpose` tag for `nonce` (constant
    /// time).
    pub fn recognises(&self, purpose: KnownPurpose, nonce: &TagNonce, tag: &KnownTag) -> bool {
        self.tag_mac(purpose, nonce)
            .is_ok_and(|mac| mac.verify_truncated_left(&tag.0).is_ok())
    }

    fn tag_mac(&self, purpose: KnownPurpose, nonce: &TagNonce) -> Result<HmacSha256> {
        MacKey::new(&self.0).tag_mac(purpose.label(), nonce)
    }
}

impl fmt::Debug for KnownSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KnownSecret(<redacted>)")
    }
}

/// Where a [`KnownTag`] is used; each purpose has its own HMAC label, so
/// a tag made for one never verifies as another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KnownPurpose {
    /// Presence adverts on the LAN (public).
    Discovery,
}

impl KnownPurpose {
    const fn label(self) -> &'static [u8] {
        match self {
            Self::Discovery => KNOWN_TAG_DISCOVERY,
        }
    }
}

/// Public tag a known device recognises in an advert (hex in TXT records).
/// A hint only, never authentication. `Debug` hides the bytes so adverts
/// cannot be linked from logs.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct KnownTag(pub [u8; KNOWN_TAG_LEN]);

impl fmt::Debug for KnownTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KnownTag(<redacted>)")
    }
}

impl fmt::Display for KnownTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex_encode(&self.0, f)
    }
}

impl FromStr for KnownTag {
    type Err = PairingError;

    fn from_str(s: &str) -> Result<Self> {
        hex_decode(s, "known tag").map(Self)
    }
}

#[cfg(test)]
#[path = "../../tests/pairing/known/unit.rs"]
mod tests;
