//! What each side brings: the receiver's [`Secret`] and the sender's
//! [`Expected`] offer.

use std::sync::Arc;

use carrova_crypto::pairing::{PairingCode, ResumeSecret, ResumeTag, TagNonce, TagPurpose};

use crate::claim::CodeClaim;
use crate::discovery::{Subject, TagProof};
use crate::error::Result;
use crate::replay::ResumeNonces;
use crate::wire::Intent;

/// What the receiver brings to the handshake.
#[derive(Debug, Clone)]
pub(crate) enum Secret {
    /// First contact: the pairing code.
    Pair(PairingCode),
    /// Reconnect: the session's resume secret.
    Resume(ResumeSecret),
}

impl Secret {
    /// What discovery looks for: the nameplate (never the words) or the
    /// resumable session.
    pub(crate) fn subject(&self) -> Subject {
        match self {
            Self::Pair(code) => Subject::Pair(code.nameplate()),
            Self::Resume(secret) => Subject::Resume(secret.clone()),
        }
    }

    /// The `Hello` intent (resume: a fresh `Hello` tag per attempt).
    pub(super) fn intent(&self) -> Result<Intent> {
        Ok(match self {
            Self::Pair(code) => Intent::Pair {
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
}

/// What the sender offers, with the listener-wide state that keeps each
/// secret's use single (cheap to clone).
#[derive(Debug, Clone)]
pub(crate) enum Expected {
    /// A pairing code and its single-use claim.
    Pair {
        code: PairingCode,
        claim: Arc<CodeClaim>,
    },
    /// A resumable session and the `Hello` nonces already used for it.
    Resume {
        secret: ResumeSecret,
        nonces: Arc<ResumeNonces>,
    },
}

impl Expected {
    /// Does a receiver's `Hello` ask for what we offer? A recognised
    /// resume `Hello` is admitted once: its nonce is recorded.
    pub(super) fn admit(&self, intent: &Intent) -> bool {
        match (self, intent) {
            (Self::Pair { code, .. }, Intent::Pair { nameplate }) => {
                code.nameplate().get() == *nameplate
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
