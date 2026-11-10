//! HMAC keys and session resumption: secrets, tags and proofs.

use std::fmt;
use std::str::FromStr;

use hmac::Mac;
use zeroize::Zeroizing;

use super::{
    ChannelBinding, Confirmation, HmacSha256, PairingError, PakeRole, RESUME_SECRET_LEN,
    RESUME_TAG_DISCOVERY, RESUME_TAG_HELLO, RESUME_TAG_LEN, Result, TAG_NONCE_LEN, hex_decode,
    hex_encode,
};

// ------------------------------------------------------------------ HMAC key

/// A 32-byte HMAC-SHA256 key (zeroized on drop).
pub(super) struct MacKey(Zeroizing<[u8; 32]>);

impl MacKey {
    pub(super) fn new(key: &[u8; 32]) -> Self {
        Self(Zeroizing::new(*key))
    }

    /// Fresh HMAC keyed with this key. `new_from_slice` accepts every key
    /// length for HMAC; the error arm exists only because the trait is
    /// shared with fixed-length MACs.
    pub(super) fn mac(&self) -> Result<HmacSha256> {
        HmacSha256::new_from_slice(&self.0[..]).map_err(|_| PairingError::KeySchedule)
    }

    /// `HMAC(key, role identity ‖ binding)`.
    pub(super) fn role_mac(&self, role: PakeRole, binding: &ChannelBinding) -> Result<HmacSha256> {
        let mut mac = self.mac()?;
        mac.update(role.identity());
        mac.update(binding.as_bytes());
        Ok(mac)
    }
}

// ------------------------------------------------------------------ resume

/// Secret that lets the two peers of one session reconnect without a new
/// code. Derived from the SPAKE2 key; stored per session.
#[derive(Clone)]
pub struct ResumeSecret(Zeroizing<[u8; RESUME_SECRET_LEN]>);

impl ResumeSecret {
    /// Wrap a freshly derived secret without copying it out of `Zeroizing`.
    pub(super) fn from_key(key: Zeroizing<[u8; RESUME_SECRET_LEN]>) -> Self {
        Self(key)
    }

    /// Restore a persisted secret.
    #[must_use]
    pub fn from_bytes(bytes: [u8; RESUME_SECRET_LEN]) -> Self {
        Self(Zeroizing::new(bytes))
    }

    /// Raw bytes, for persisting only.
    #[must_use]
    pub fn expose_bytes(&self) -> &[u8; RESUME_SECRET_LEN] {
        &self.0
    }

    fn key(&self) -> MacKey {
        MacKey::new(&self.0)
    }

    /// Tag of this session for one `purpose` and `nonce`:
    /// `HMAC(secret, label(purpose) ‖ nonce)[..16]`. A fresh [`TagNonce`]
    /// per use makes the tags of one session unlinkable for anyone without
    /// the secret; the peer recognises the session by recomputing the tag.
    /// Distinct labels per [`TagPurpose`] keep a public discovery tag from
    /// passing as a `Hello` tag.
    pub fn tag_for(&self, purpose: TagPurpose, nonce: &TagNonce) -> Result<ResumeTag> {
        let full = self.tag_mac(purpose, nonce)?.finalize().into_bytes();
        let mut tag = [0u8; RESUME_TAG_LEN];
        tag.copy_from_slice(&full[..RESUME_TAG_LEN]);
        Ok(ResumeTag(tag))
    }

    /// Whether `tag` is this secret's `purpose` tag for `nonce` (constant
    /// time).
    pub fn recognises(&self, purpose: TagPurpose, nonce: &TagNonce, tag: &ResumeTag) -> bool {
        self.tag_mac(purpose, nonce)
            .is_ok_and(|mac| mac.verify_truncated_left(&tag.0).is_ok())
    }

    fn tag_mac(&self, purpose: TagPurpose, nonce: &TagNonce) -> Result<HmacSha256> {
        let mut mac = self.key().mac()?;
        mac.update(purpose.label());
        mac.update(&nonce.0);
        Ok(mac)
    }

    /// This side's proof of the secret over a fresh connection's binding.
    pub fn prove(&self, role: PakeRole, binding: &ChannelBinding) -> Result<Confirmation> {
        Ok(Confirmation(
            self.key()
                .role_mac(role, binding)?
                .finalize()
                .into_bytes()
                .into(),
        ))
    }

    /// Check the peer's proof in constant time.
    pub fn verify(
        &self,
        peer_role: PakeRole,
        binding: &ChannelBinding,
        proof: &Confirmation,
    ) -> Result<()> {
        self.key()
            .role_mac(peer_role, binding)?
            .verify_slice(&proof.0)
            .map_err(|_| PairingError::ConfirmationFailed)
    }
}

impl fmt::Debug for ResumeSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ResumeSecret(<redacted>)")
    }
}

/// Public tag identifying a resumable session in discovery (hex in TXT
/// records).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ResumeTag(pub [u8; RESUME_TAG_LEN]);

impl fmt::Display for ResumeTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex_encode(&self.0, f)
    }
}

impl FromStr for ResumeTag {
    type Err = PairingError;

    fn from_str(s: &str) -> Result<Self> {
        hex_decode(s, "resume tag").map(Self)
    }
}

serde_via_str!(ResumeTag);

/// Where a [`ResumeTag`] is used; each purpose has its own HMAC label, so
/// a tag made for one never verifies as the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagPurpose {
    /// Discovery: a sender advertisement or a receiver query (public on
    /// the LAN).
    Discovery,
    /// The receiver's resume `Hello` (sent only inside the encrypted
    /// connection).
    Hello,
}

impl TagPurpose {
    const fn label(self) -> &'static [u8] {
        match self {
            Self::Discovery => RESUME_TAG_DISCOVERY,
            Self::Hello => RESUME_TAG_HELLO,
        }
    }
}

/// Random nonce of one resume advertisement (public; see
/// [`ResumeSecret::tag_for`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TagNonce(pub [u8; TAG_NONCE_LEN]);

impl TagNonce {
    /// Fresh nonce from the OS RNG.
    pub fn random() -> Result<Self> {
        let mut nonce = [0u8; TAG_NONCE_LEN];
        getrandom::fill(&mut nonce).map_err(|_| PairingError::Rng)?;
        Ok(Self(nonce))
    }
}

impl fmt::Display for TagNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        hex_encode(&self.0, f)
    }
}

impl FromStr for TagNonce {
    type Err = PairingError;

    fn from_str(s: &str) -> Result<Self> {
        hex_decode(s, "resume tag nonce").map(Self)
    }
}
