//! What each side brings: the receiver's [`Secret`] and the sender's
//! [`Expected`] offer.

use std::sync::Arc;

use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::{
    LinkTag, PairingCode, ResumeSecret, ResumeTag, TagNonce, TagPurpose,
};

use super::PeerIdentity;
use crate::claim::CodeClaim;
use crate::discovery::{Subject, TagProof};
use crate::error::Result;
use crate::replay::ResumeNonces;
use crate::wire::{CloseCode, Intent, PairPurpose};

/// What the receiver brings to the handshake.
#[derive(Debug, Clone)]
pub(crate) enum Secret {
    /// First contact: the pairing code, and this device's key if it
    /// remembers devices (always, for pair-only).
    Pair {
        code: PairingCode,
        device: Option<Arc<DeviceKey>>,
        purpose: PairPurpose,
        /// The sender's key from a scanned QR code: its `Identify` must
        /// carry exactly this key.
        pin: Option<DevicePublic>,
    },
    /// Reconnect: the session's resume secret.
    Resume(ResumeSecret),
    /// Link to a paired device: its stored key and shared secret, and this
    /// device's key.
    Link {
        peer: PeerIdentity,
        device: Arc<DeviceKey>,
    },
}

impl Secret {
    /// What discovery looks for: the nameplate (never the words) or the
    /// resumable session; a link is dialled at a known address.
    pub(crate) fn subject(&self) -> Option<Subject> {
        match self {
            Self::Pair { code, .. } => Some(Subject::Pair(code.nameplate())),
            Self::Resume(secret) => Some(Subject::Resume(secret.clone())),
            Self::Link { .. } => None,
        }
    }

    /// The `Hello` intent (resume and link: a fresh tag per attempt).
    pub(super) fn intent(&self) -> Result<Intent> {
        Ok(match self {
            Self::Pair { code, .. } => Intent::Pair {
                nameplate: code.nameplate().get(),
            },
            Self::Resume(secret) => {
                let proof = TagProof::fresh(secret, TagPurpose::Hello)?;
                Intent::Resume {
                    nonce: proof.nonce.0,
                    tag: proof.tag.0,
                }
            }
            Self::Link { peer, device } => {
                let nonce = TagNonce::random()?;
                let tag = peer.secret.link_tag(&nonce, &device.public())?;
                Intent::Link {
                    nonce: nonce.0,
                    tag: tag.0,
                }
            }
        })
    }

    /// This device's key, when it remembers devices and pairs with a code
    /// (always, for a link).
    pub(super) fn device(&self) -> Option<&DeviceKey> {
        match self {
            Self::Pair { device, .. } => device.as_deref(),
            Self::Resume(_) => None,
            Self::Link { device, .. } => Some(device),
        }
    }

    /// What the pairing is for (resume and link: the `Hello` default).
    pub(super) fn purpose(&self) -> PairPurpose {
        match self {
            Self::Pair { purpose, .. } => *purpose,
            Self::Resume(_) | Self::Link { .. } => PairPurpose::Move,
        }
    }

    /// Whether the sender must prove its device identity (pair-only and
    /// link; a move pairing identifies only if the sender remembers
    /// devices).
    pub(super) fn must_identify(&self) -> bool {
        matches!(self, Self::Link { .. }) || self.purpose() == PairPurpose::PairOnly
    }

    /// The sender key pinned by a scanned QR code, if any.
    pub(super) fn pin(&self) -> Option<&DevicePublic> {
        match self {
            Self::Pair { pin, .. } => pin.as_ref(),
            Self::Resume(_) | Self::Link { .. } => None,
        }
    }
}

/// What the sender offers, with the listener-wide state that keeps each
/// secret's use single (cheap to clone).
#[derive(Debug, Clone)]
pub(crate) enum Expected {
    /// A pairing code and its single-use claim, this device's key if it
    /// remembers devices (always, for pair-only), and what the code is
    /// offered for.
    Pair {
        code: PairingCode,
        claim: Arc<CodeClaim>,
        device: Option<Arc<DeviceKey>>,
        purpose: PairPurpose,
    },
    /// A resumable session and the `Hello` nonces already used for it.
    Resume {
        secret: ResumeSecret,
        nonces: Arc<ResumeNonces>,
    },
    /// Links from known peers (at most [`MAX_LINK_CANDIDATES`], most
    /// recently used first), this device's key, and the `Hello` nonces
    /// already used (shared with resume).
    ///
    /// [`MAX_LINK_CANDIDATES`]: crate::tuning::MAX_LINK_CANDIDATES
    Link {
        peers: Arc<[PeerIdentity]>,
        device: Arc<DeviceKey>,
        nonces: Arc<ResumeNonces>,
    },
}

/// A receiver's `Hello` the sender takes on.
pub(super) enum Admitted {
    /// It asks for the code or session offered.
    Offer,
    /// It links from this known peer.
    Link(PeerIdentity),
}

impl Expected {
    /// Does a receiver's `Hello` ask for what we offer? A pairing `Hello`
    /// must name our nameplate and purpose, and a pair-only one must ask
    /// for identities. A recognised resume or link `Hello` is admitted
    /// once: its nonce is recorded (an unrecognised one's is not).
    pub(super) fn admit(
        &self,
        intent: &Intent,
        identify: bool,
        asked: PairPurpose,
    ) -> Option<Admitted> {
        match (self, intent) {
            (Self::Pair { code, purpose, .. }, Intent::Pair { nameplate }) => {
                (code.nameplate().get() == *nameplate
                    && *purpose == asked
                    && (identify || asked == PairPurpose::Move))
                    .then_some(Admitted::Offer)
            }
            (Self::Resume { secret, nonces }, Intent::Resume { nonce, tag }) => {
                let proof = TagProof {
                    nonce: TagNonce(*nonce),
                    tag: ResumeTag(*tag),
                };
                (proof.made_with(secret, TagPurpose::Hello) && nonces.first_use(proof.nonce))
                    .then_some(Admitted::Offer)
            }
            (Self::Link { peers, nonces, .. }, Intent::Link { nonce, tag }) if identify => {
                let nonce = TagNonce(*nonce);
                let peer = recognise(peers, &nonce, &LinkTag(*tag))?;
                nonces
                    .first_use(nonce)
                    .then(|| Admitted::Link(peer.clone()))
            }
            _ => None,
        }
    }

    /// This device's key, when it remembers devices and pairs with a code
    /// (always, for links).
    pub(super) fn device(&self) -> Option<&DeviceKey> {
        match self {
            Self::Pair { device, .. } => device.as_deref(),
            Self::Resume { .. } => None,
            Self::Link { device, .. } => Some(device),
        }
    }

    /// How an attempt that did not authenticate is closed: a link always
    /// with `LinkFailed` (no detail for the peer, ADR 0002 §3); otherwise
    /// `PairingFailed` once the secret was at stake, else `Rejected`.
    pub(super) fn failure_code(&self, staked: bool) -> CloseCode {
        match self {
            Self::Link { .. } => CloseCode::LinkFailed,
            _ if staked => CloseCode::PairingFailed,
            _ => CloseCode::Rejected,
        }
    }

    /// Resolves once an attempt put the secret at stake: pairing with the
    /// code claimed (never for resume, whose tag is public).
    pub(crate) async fn staked(&self) {
        match self {
            Self::Pair { claim, .. } => claim.taken().await,
            Self::Resume { .. } | Self::Link { .. } => std::future::pending().await,
        }
    }

    /// Whether an attempt has put the secret at stake and not finished:
    /// pairing with the code claimed (the accept loop is never left
    /// before such an attempt ends).
    pub(crate) fn at_stake(&self) -> bool {
        match self {
            Self::Pair { claim, .. } => claim.is_taken(),
            Self::Resume { .. } | Self::Link { .. } => false,
        }
    }
}

/// The peer whose secret made `tag` for `nonce` with its own key. Every
/// candidate is checked (each in constant time, no early exit), so timing
/// does not tell which one matched.
fn recognise<'a>(
    peers: &'a [PeerIdentity],
    nonce: &TagNonce,
    tag: &LinkTag,
) -> Option<&'a PeerIdentity> {
    peers.iter().fold(None, |found, peer| {
        let hit = peer.secret.recognises_link(nonce, &peer.public, tag);
        found.or(hit.then_some(peer))
    })
}
