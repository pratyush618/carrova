//! Known-device secret, its discovery tags (ADR 0001 §4–5) and its link
//! tags and proofs (ADR 0002 §3).

use std::fmt;
use std::str::FromStr;

use hmac::Mac;
use zeroize::Zeroizing;

use super::keys::MacKey;
use super::{
    ChannelBinding, Confirmation, HmacSha256, KNOWN_SECRET_LEN, KNOWN_TAG_DISCOVERY, KNOWN_TAG_LEN,
    KNOWN_TAG_LINK, LINK_CONFIRM, LINK_TAG_LEN, PairingError, PakeRole, Result, TagNonce,
    hex_decode, hex_encode,
};
use crate::identity::DevicePublic;

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

    /// Tag the device `advertiser` puts in its advert, for one `purpose`
    /// and `nonce`: `HMAC(secret, label(purpose) ‖ nonce ‖ advertiser)[..8]`.
    /// A fresh [`TagNonce`] per advert keeps tags unlinkable for anyone
    /// without the secret; binding the advertiser's key means a device never
    /// mistakes its own advert for its peer's (both hold the secret).
    pub fn tag_for(
        &self,
        purpose: KnownPurpose,
        nonce: &TagNonce,
        advertiser: &DevicePublic,
    ) -> Result<KnownTag> {
        let full = self
            .tag_mac(purpose.label(), nonce, advertiser)?
            .finalize()
            .into_bytes();
        let mut tag = [0u8; KNOWN_TAG_LEN];
        tag.copy_from_slice(&full[..KNOWN_TAG_LEN]);
        Ok(KnownTag(tag))
    }

    /// Whether `tag` is this secret's `purpose` tag for `nonce`, made by
    /// `advertiser` (constant time).
    pub fn recognises(
        &self,
        purpose: KnownPurpose,
        nonce: &TagNonce,
        advertiser: &DevicePublic,
        tag: &KnownTag,
    ) -> bool {
        self.tag_mac(purpose.label(), nonce, advertiser)
            .is_ok_and(|mac| mac.verify_truncated_left(&tag.0).is_ok())
    }

    /// Tag the `dialer` puts in a link `Hello`:
    /// `HMAC(secret, "carrova/1/known-link" ‖ nonce ‖ dialer)[..16]`.
    pub fn link_tag(&self, nonce: &TagNonce, dialer: &DevicePublic) -> Result<LinkTag> {
        let full = self
            .tag_mac(KNOWN_TAG_LINK, nonce, dialer)?
            .finalize()
            .into_bytes();
        let mut tag = [0u8; LINK_TAG_LEN];
        tag.copy_from_slice(&full[..LINK_TAG_LEN]);
        Ok(LinkTag(tag))
    }

    /// Whether `tag` is this secret's link tag for `nonce`, made by
    /// `dialer` (constant time).
    #[must_use]
    pub fn recognises_link(&self, nonce: &TagNonce, dialer: &DevicePublic, tag: &LinkTag) -> bool {
        self.tag_mac(KNOWN_TAG_LINK, nonce, dialer)
            .is_ok_and(|mac| mac.verify_truncated_left(&tag.0).is_ok())
    }

    /// This side's link proof over a fresh connection's binding:
    /// `HMAC(secret, "carrova/1/link-confirm" ‖ role ‖ binding)`.
    pub fn prove(&self, role: PakeRole, binding: &ChannelBinding) -> Result<Confirmation> {
        self.key().prove(LINK_CONFIRM, role, binding)
    }

    /// Check the peer's link proof in constant time.
    pub fn verify(
        &self,
        peer_role: PakeRole,
        binding: &ChannelBinding,
        proof: &Confirmation,
    ) -> Result<()> {
        self.key().verify(LINK_CONFIRM, peer_role, binding, proof)
    }

    fn key(&self) -> MacKey {
        MacKey::new(&self.0)
    }

    /// `HMAC(secret, label ‖ nonce ‖ device)`, untruncated.
    fn tag_mac(
        &self,
        label: &[u8],
        nonce: &TagNonce,
        advertiser: &DevicePublic,
    ) -> Result<HmacSha256> {
        let mut mac = self.key().tag_mac(label, nonce)?;
        mac.update(advertiser.as_bytes());
        Ok(mac)
    }
}

impl fmt::Debug for KnownSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KnownSecret(<redacted>)")
    }
}

/// Where a [`KnownTag`] is used; each purpose has its own HMAC label, so
/// a tag made for one never verifies as another. Link tags are not a
/// purpose: they exist only at full length ([`KnownSecret::link_tag`]).
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

/// Tag naming a known pair in a link `Hello` (ADR 0002 §3). Its own type
/// so it is never mixed up with a [`KnownTag`]; `Debug` hides the bytes.
/// No `PartialEq`: compare only through [`KnownSecret::recognises_link`]
/// (constant time).
#[derive(Clone, Copy)]
pub struct LinkTag(pub [u8; LINK_TAG_LEN]);

impl fmt::Debug for LinkTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("LinkTag(<redacted>)")
    }
}

#[cfg(test)]
#[path = "../../tests/pairing/known/unit.rs"]
mod tests;
