//! Presence: a device announcing itself on the LAN outside any session
//! (ADR 0001 §5). Devices it knows find it by a [`KnownTags`] value only
//! they can recognise; when the user shows it to everyone it also gives
//! its name and OS ([`PresenceAdvert`]).

use std::fmt;

use carrova_crypto::pairing::{
    KNOWN_TAG_LEN, KnownPurpose, KnownSecret, KnownTag, TAG_NONCE_LEN, TagNonce,
};
use carrova_platform::{OsKind, Platform};

use super::advertise::{Advertiser, txt_fit};
use crate::error::Result;
use crate::{BROADCAST_PORT, TXT_DEVICE_NAME, TXT_KNOWN_TAGS, TXT_OS};

/// Most tags in one advert: a nonce and 13 tags in hex fill one 255-byte
/// TXT string (`kt=` + 32 + 13 × 16 = 243 bytes).
pub const MAX_KNOWN_TAGS: usize = 13;

/// One advert's known-device tags: a fresh random nonce shared by all of
/// them and one [`KnownPurpose::Discovery`] tag per known device. Without
/// a matching secret, two adverts cannot be linked. A hint, never
/// authentication. `Debug` hides nonce and tags.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct KnownTags {
    nonce: TagNonce,
    tags: Vec<KnownTag>,
}

impl KnownTags {
    /// Tags for the first [`MAX_KNOWN_TAGS`] of `secrets` (callers put the
    /// most recently used first) under a fresh nonce; `None` if there are
    /// none.
    ///
    /// # Errors
    ///
    /// The OS random generator failed.
    pub fn fresh<'a>(secrets: impl IntoIterator<Item = &'a KnownSecret>) -> Result<Option<Self>> {
        let nonce = TagNonce::random()?;
        let tags = secrets
            .into_iter()
            .take(MAX_KNOWN_TAGS)
            .map(|secret| secret.tag_for(KnownPurpose::Discovery, &nonce))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok((!tags.is_empty()).then_some(Self { nonce, tags }))
    }

    /// Whether one of the tags was made with `secret` (constant time per
    /// tag).
    #[must_use]
    pub fn made_with(&self, secret: &KnownSecret) -> bool {
        self.tags
            .iter()
            .any(|tag| secret.recognises(KnownPurpose::Discovery, &self.nonce, tag))
    }

    /// `<nonce hex><tag hex>…`.
    pub(crate) fn encode(&self) -> String {
        self.tags
            .iter()
            .fold(self.nonce.to_string(), |hex, tag| hex + &tag.to_string())
    }

    /// Parse [`Self::encode`]'s form; `None` unless well formed with 1 to
    /// [`MAX_KNOWN_TAGS`] tags.
    pub(crate) fn parse(value: &str) -> Option<Self> {
        let (nonce, tags) = value.split_at_checked(2 * TAG_NONCE_LEN)?;
        let tag_hex = 2 * KNOWN_TAG_LEN;
        if !tags.is_ascii() || tags.is_empty() || tags.len() % tag_hex != 0 {
            return None;
        }
        let tags = tags
            .as_bytes()
            .chunks(tag_hex)
            .map(|hex| std::str::from_utf8(hex).ok()?.parse().ok())
            .collect::<Option<Vec<KnownTag>>>()?;
        (tags.len() <= MAX_KNOWN_TAGS).then_some(Self {
            nonce: nonce.parse().ok()?,
            tags,
        })
    }
}

impl fmt::Debug for KnownTags {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("KnownTags(<redacted>)")
    }
}

/// What a device announces about itself: known-device tags, and its name
/// and OS when it shows itself to everyone (also, from a sender offering a
/// code, just its name). Read from the network, every field is
/// peer-provided: sanitise the name before display and never treat it as
/// proof of who sent it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct PresenceAdvert {
    /// Tags for the devices it knows ([`TXT_KNOWN_TAGS`]).
    pub known: Option<KnownTags>,
    /// Its device name ([`TXT_DEVICE_NAME`]).
    pub name: Option<String>,
    /// Its OS family ([`TXT_OS`]).
    pub os: Option<OsKind>,
}

impl PresenceAdvert {
    /// The TXT entries, in key order `kt`, `dn`, `os`, absent fields left
    /// out (the name cut to fit one entry).
    pub(crate) fn txt(&self) -> Vec<(&'static str, String)> {
        [
            self.known.as_ref().map(|k| (TXT_KNOWN_TAGS, k.encode())),
            self.name
                .as_deref()
                .map(|n| (TXT_DEVICE_NAME, txt_fit(n).to_owned())),
            self.os.map(|os| (TXT_OS, os.as_str().to_owned())),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    /// Read an advert from TXT lookups (`get(key)`); malformed fields are
    /// left out.
    pub(crate) fn from_txt<'a>(get: impl Fn(&str) -> Option<&'a str>) -> Self {
        Self {
            known: get(TXT_KNOWN_TAGS).and_then(KnownTags::parse),
            name: get(TXT_DEVICE_NAME).map(str::to_owned),
            os: get(TXT_OS).and_then(|os| os.parse().ok()),
        }
    }
}

/// This device's presence on the LAN (mDNS and broadcast responder) until
/// dropped.
#[derive(Debug)]
pub struct PresenceBeacon(Advertiser);

impl PresenceBeacon {
    /// Start announcing `advert`. Parts that fail to start are logged and
    /// skipped, as for a session's advertisement.
    ///
    /// # Errors
    ///
    /// No async runtime, or the OS random generator failed.
    pub fn start(platform: &dyn Platform, advert: PresenceAdvert) -> Result<Self> {
        // A random instance: nothing ties it to the device's identity.
        let instance = format!("carrova-{}", &TagNonce::random()?.to_string()[..12]);
        // shortcut: no request listener yet, so the advert names the
        // broadcast responder's port; the request listener (Phase 3) passes its own.
        let mut advertiser = Advertiser::start(platform, BROADCAST_PORT, instance, None)?;
        advertiser.set_presence(advert);
        Ok(Self(advertiser))
    }

    /// Announce `advert` instead (callers pass fresh [`KnownTags`]).
    pub fn set(&mut self, advert: PresenceAdvert) {
        self.0.set_presence(advert);
    }
}

#[cfg(test)]
#[path = "../../tests/discovery/presence/unit.rs"]
mod tests;
