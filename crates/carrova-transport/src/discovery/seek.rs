//! The discovery key: what a sender advertises and a receiver asks for.
//!
//! On the wire a key is a TXT key and value (mDNS) or `key=value` after
//! [`BROADCAST_QUERY`] (broadcast). Keys are parsed into a typed [`Seek`]
//! and compared as values, never as strings; a value is accepted only in
//! its canonical encoding.

use carrova_crypto::pairing::{
    Nameplate, RESUME_TAG_LEN, ResumeSecret, ResumeTag, TAG_NONCE_LEN, TagNonce, TagPurpose,
};

use crate::error::Result;
use crate::{BROADCAST_QUERY, TXT_NAMEPLATE, TXT_RESUME_TAG};

/// One public resume tag with the nonce it was computed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TagProof {
    pub(crate) nonce: TagNonce,
    pub(crate) tag: ResumeTag,
}

impl TagProof {
    /// Length of [`Self::encode`]: nonce and tag, two hex digits a byte.
    const ENCODED_LEN: usize = 2 * (TAG_NONCE_LEN + RESUME_TAG_LEN);

    /// A fresh proof (new random nonce) for `secret` and `purpose`.
    pub(crate) fn fresh(secret: &ResumeSecret, purpose: TagPurpose) -> Result<Self> {
        let nonce = TagNonce::random()?;
        Ok(Self {
            tag: secret.tag_for(purpose, &nonce)?,
            nonce,
        })
    }

    /// Whether this proof was made with `secret` for `purpose` (constant
    /// time).
    pub(crate) fn made_with(&self, secret: &ResumeSecret, purpose: TagPurpose) -> bool {
        secret.recognises(purpose, &self.nonce, &self.tag)
    }

    /// `<nonce hex><tag hex>`.
    fn encode(&self) -> String {
        format!("{}{}", self.nonce, self.tag)
    }

    fn parse(value: &str) -> Option<Self> {
        if !value.is_char_boundary(2 * TAG_NONCE_LEN) || value.len() != Self::ENCODED_LEN {
            return None;
        }
        let (nonce, tag) = value.split_at(2 * TAG_NONCE_LEN);
        Some(Self {
            nonce: nonce.parse().ok()?,
            tag: tag.parse().ok()?,
        })
    }
}

/// Which kind of key: the TXT key / query key on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SeekKey {
    /// [`TXT_NAMEPLATE`].
    Nameplate,
    /// [`TXT_RESUME_TAG`].
    ResumeTag,
}

impl SeekKey {
    /// The key's wire spelling.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Nameplate => TXT_NAMEPLATE,
            Self::ResumeTag => TXT_RESUME_TAG,
        }
    }

    fn from_wire(key: &str) -> Option<Self> {
        [Self::Nameplate, Self::ResumeTag]
            .into_iter()
            .find(|k| k.as_str() == key)
    }
}

/// A parsed discovery key and value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Seek {
    /// The sender offering a pairing code with this nameplate.
    Pairing(Nameplate),
    /// The sender of a resumable session, named by a rotating
    /// [`TagPurpose::Discovery`] tag.
    Resume(TagProof),
}

impl Seek {
    /// The key this value goes under.
    pub(crate) const fn key(&self) -> SeekKey {
        match self {
            Self::Pairing(_) => SeekKey::Nameplate,
            Self::Resume(_) => SeekKey::ResumeTag,
        }
    }

    /// The value's wire encoding (nameplate in decimal, resume
    /// `<nonce hex><tag hex>`).
    pub(crate) fn value(&self) -> String {
        match self {
            Self::Pairing(np) => np.to_string(),
            Self::Resume(proof) => proof.encode(),
        }
    }

    /// Parse `value` under `key`; `None` unless well formed. A nameplate
    /// must be in canonical decimal (no padding, sign or leading zeros);
    /// resume hex is case-insensitive.
    pub(crate) fn parse(key: SeekKey, value: &str) -> Option<Self> {
        match key {
            SeekKey::Nameplate => {
                let np: Nameplate = value.parse().ok()?;
                (np.to_string() == value).then_some(Self::Pairing(np))
            }
            SeekKey::ResumeTag => TagProof::parse(value).map(Self::Resume),
        }
    }

    /// Broadcast query datagram: [`BROADCAST_QUERY`] ‖ `key=value`.
    pub(crate) fn to_query(self) -> Vec<u8> {
        let mut q = BROADCAST_QUERY.to_vec();
        q.extend_from_slice(format!("{}={}", self.key().as_str(), self.value()).as_bytes());
        q
    }

    /// Parse a broadcast query datagram.
    pub(crate) fn from_query(datagram: &[u8]) -> Option<Self> {
        let (key, value) = std::str::from_utf8(datagram.strip_prefix(BROADCAST_QUERY)?)
            .ok()?
            .split_once('=')?;
        Self::parse(SeekKey::from_wire(key)?, value)
    }
}

/// What a discovery exchange is about: the sender's offer, or what the
/// receiver looks for.
#[derive(Debug, Clone)]
pub(crate) enum Subject {
    /// A pairing code with this nameplate (never the words).
    Pair(Nameplate),
    /// The resumable session holding this secret.
    Resume(ResumeSecret),
}

impl Subject {
    /// The key a [`Seek`] for this subject goes under.
    pub(crate) const fn key(&self) -> SeekKey {
        match self {
            Self::Pair(_) => SeekKey::Nameplate,
            Self::Resume(_) => SeekKey::ResumeTag,
        }
    }

    /// A key naming this subject (resume: a fresh tag on every call, so
    /// keys cannot be linked without the secret).
    pub(crate) fn fresh_seek(&self) -> Result<Seek> {
        Ok(match self {
            Self::Pair(np) => Seek::Pairing(*np),
            Self::Resume(secret) => Seek::Resume(TagProof::fresh(secret, TagPurpose::Discovery)?),
        })
    }

    /// Does `seek` name this subject? Resume tags are checked in constant
    /// time.
    pub(crate) fn recognises(&self, seek: &Seek) -> bool {
        match (self, seek) {
            (Self::Pair(np), Seek::Pairing(other)) => np == other,
            (Self::Resume(secret), Seek::Resume(proof)) => {
                proof.made_with(secret, TagPurpose::Discovery)
            }
            _ => false,
        }
    }
}

/// What a sender offers right now: the subject and the key it is
/// advertised under (fixed for the advertisement's lifetime).
#[derive(Debug, Clone)]
pub(crate) struct Offer {
    subject: Subject,
    advert: Seek,
}

impl Offer {
    /// Offer `subject` under a fresh advertisement key.
    pub(crate) fn new(subject: Subject) -> Result<Self> {
        Ok(Self {
            advert: subject.fresh_seek()?,
            subject,
        })
    }

    /// The advertised key.
    pub(crate) const fn advert(&self) -> &Seek {
        &self.advert
    }

    /// Whether this offers a pairing code (not a resumable session).
    pub(crate) const fn is_pairing(&self) -> bool {
        matches!(self.subject, Subject::Pair(_))
    }

    /// Does a broadcast query ask for this offer?
    pub(crate) fn answers(&self, query: &[u8]) -> bool {
        Seek::from_query(query).is_some_and(|seek| self.subject.recognises(&seek))
    }
}

#[cfg(test)]
#[path = "../../tests/discovery/seek/unit.rs"]
mod tests;
