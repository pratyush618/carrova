//! What each side brings: the receiver's [`Secret`] and the sender's
//! [`Expected`] offer.

use std::sync::Arc;

use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::{PairingCode, ResumeSecret, ResumeTag, TagNonce, TagPurpose};

use crate::claim::CodeClaim;
use crate::discovery::{Subject, TagProof};
use crate::error::Result;
use crate::replay::ResumeNonces;
use crate::wire::{Intent, PairPurpose};

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
}

impl Secret {
    /// What discovery looks for: the nameplate (never the words) or the
    /// resumable session.
    pub(crate) fn subject(&self) -> Subject {
        match self {
            Self::Pair { code, .. } => Subject::Pair(code.nameplate()),
            Self::Resume(secret) => Subject::Resume(secret.clone()),
        }
    }

    /// The `Hello` intent (resume: a fresh `Hello` tag per attempt).
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
        })
    }

    /// This device's key, when it remembers devices and pairs with a code.
    pub(super) fn device(&self) -> Option<&DeviceKey> {
        match self {
            Self::Pair { device, .. } => device.as_deref(),
            Self::Resume(_) => None,
        }
    }

    /// What the pairing is for (a resume always continues a move).
    pub(super) fn purpose(&self) -> PairPurpose {
        match self {
            Self::Pair { purpose, .. } => *purpose,
            Self::Resume(_) => PairPurpose::Move,
        }
    }

    /// The sender key pinned by a scanned QR code, if any.
    pub(super) fn pin(&self) -> Option<&DevicePublic> {
        match self {
            Self::Pair { pin, .. } => pin.as_ref(),
            Self::Resume(_) => None,
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
}

impl Expected {
    /// Does a receiver's `Hello` ask for what we offer? A pairing `Hello`
    /// must name our nameplate and purpose, and a pair-only one must ask
    /// for identities. A recognised resume `Hello` is admitted once: its
    /// nonce is recorded.
    pub(super) fn admit(&self, intent: &Intent, identify: bool, asked: PairPurpose) -> bool {
        match (self, intent) {
            (Self::Pair { code, purpose, .. }, Intent::Pair { nameplate }) => {
                code.nameplate().get() == *nameplate
                    && *purpose == asked
                    && (identify || asked == PairPurpose::Move)
            }
            (Self::Resume { secret, nonces }, Intent::Resume { nonce, tag }) => {
                let proof = TagProof {
                    nonce: TagNonce(*nonce),
                    tag: ResumeTag(*tag),
                };
                proof.made_with(secret, TagPurpose::Hello) && nonces.first_use(proof.nonce)
            }
            _ => false,
        }
    }

    /// This device's key, when it remembers devices and pairs with a code.
    pub(super) fn device(&self) -> Option<&DeviceKey> {
        match self {
            Self::Pair { device, .. } => device.as_deref(),
            Self::Resume { .. } => None,
        }
    }

    /// Resolves once an attempt put the secret at stake: pairing with the
    /// code claimed (never for resume, whose tag is public).
    pub(crate) async fn staked(&self) {
        match self {
            Self::Pair { claim, .. } => claim.taken().await,
            Self::Resume { .. } => std::future::pending().await,
        }
    }

    /// Whether an attempt has put the secret at stake and not finished:
    /// pairing with the code claimed (the accept loop is never left
    /// before such an attempt ends).
    pub(crate) fn at_stake(&self) -> bool {
        match self {
            Self::Pair { claim, .. } => claim.is_taken(),
            Self::Resume { .. } => false,
        }
    }
}
