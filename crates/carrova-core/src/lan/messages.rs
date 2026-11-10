//! Control-stream messages of a LAN session and the receiver's inventory.

use std::fmt;

use carrova_platform::{OsKind, Platform, SemanticRoot};
use carrova_recipe::Recipe;
use serde::{Deserialize, Serialize};

use super::summary::PlanSummary;
use crate::checklist::Checklist;

/// One control-stream message of a LAN session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LanMsg {
    /// Receiver's inventory, sent right after pairing.
    Inventory(Inventory),
    /// One part of the sender's manifest CBOR (`carrova_manifest` wire
    /// format). A manifest may exceed one frame, so it is sent in parts of
    /// at most [`super::MANIFEST_PART_LEN`] bytes; `total_len` (the same in every
    /// part, at most [`carrova_manifest::DEFAULT_MAX_MANIFEST_BYTES`]) says
    /// when it is complete.
    Manifest {
        /// Length of the whole manifest CBOR.
        total_len: u64,
        /// The next bytes of it.
        #[serde(with = "serde_bytes")]
        part: Vec<u8>,
    },
    /// Receiver's plan, summarised for display on the sender.
    PlanSummary(PlanSummary),
    /// Receiver's first message on a resumed connection: where it is.
    Resume {
        /// The receiver's protocol stage.
        stage: ReceiverStage,
    },
    /// Receiver is about to pull a batch of chunks: the sender serves until
    /// that fetch ends.
    Fetch,
    /// Receiver finished applying; its end-of-session checklist (item keys
    /// and reasons only).
    Applied(Checklist),
    /// The session ends early on the sending side of this message.
    Abort {
        /// Why.
        reason: AbortReason,
    },
}

/// Where a resuming receiver is in the protocol (see [`LanMsg::Resume`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiverStage {
    /// The receiver has the manifest and is planning; a plan summary
    /// follows on approval.
    Planning,
    /// The plan is approved: chunk fetches (and finally the checklist)
    /// follow.
    Transferring,
}

/// What the receiver has: shown on the sender before it sends a manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Inventory {
    /// Receiver OS family.
    pub os: OsKind,
    /// Receiver Carrova version (`CARGO_PKG_VERSION`).
    pub carrova_version: String,
    /// Apps detected on the receiver.
    pub apps: Vec<InventoryApp>,
    /// Free bytes on the filesystem of the receiver's `user_data` root,
    /// if known.
    pub free_space: Option<u64>,
    /// The receiver's device name as it announces it (untrusted: read it
    /// through [`Self::device_name`]). Absent from older receivers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_name: Option<String>,
}

impl Inventory {
    /// This machine's inventory: OS, Carrova version, the apps `recipes`
    /// detect here (versions `None` in v1) and the free space of the
    /// `user_data` root's filesystem (its nearest existing ancestor if the
    /// root does not exist yet; `None` if unknown).
    #[must_use]
    pub fn collect(platform: &dyn Platform, recipes: &[Recipe]) -> Self {
        let apps = recipes
            .iter()
            .filter(|r| carrova_recipe::detect(r, platform).detected)
            .map(|r| InventoryApp {
                id: r.id.clone(),
                version: None,
            })
            .collect();
        let free_space = platform
            .resolve_root(SemanticRoot::UserData)
            .ok()
            .and_then(|root| crate::space::nearest_existing(platform, &root))
            .and_then(|p| platform.free_space(&p).ok());
        Self {
            os: platform.os(),
            carrova_version: env!("CARGO_PKG_VERSION").into(),
            apps,
            free_space,
            device_name: crate::about::device_name(platform),
        }
    }

    /// The announced device name, safe to display and store: control
    /// and bidi characters dropped, at most [`MAX_DEVICE_NAME`]
    /// characters, `None` if nothing is left.
    #[must_use]
    pub fn device_name(&self) -> Option<String> {
        let clean: String = self
            .device_name
            .as_deref()?
            .chars()
            .filter(|c| !carrova_manifest::is_unsafe_char(*c))
            .take(MAX_DEVICE_NAME)
            .collect();
        let clean = clean.trim();
        (!clean.is_empty()).then(|| clean.to_owned())
    }
}

/// Longest peer device name kept (characters).
pub const MAX_DEVICE_NAME: usize = 64;

/// One app detected on the receiver.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InventoryApp {
    /// Recipe id.
    pub id: String,
    /// Installed version; `None` in v1 (recipes have no version probe yet).
    pub version: Option<String>,
}

/// Why a LAN session was ended early.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AbortReason {
    /// The user cancelled.
    Cancelled,
    /// The receiver only planned (dry run): nothing was transferred.
    DryRun,
    /// The session failed on that side (message holds no secrets or file
    /// contents).
    Failed {
        /// Human-readable cause.
        message: String,
    },
}

/// Longest peer-supplied abort message kept (characters).
const MAX_PEER_MESSAGE: usize = 300;

/// Human-readable description for errors and reports. A peer's message is
/// untrusted: control and bidi characters are dropped and it is truncated.
impl fmt::Display for AbortReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cancelled => f.write_str("the other machine cancelled the session"),
            Self::DryRun => f.write_str("the receiver did a dry run; nothing was transferred"),
            Self::Failed { message } => {
                let clean: String = message
                    .chars()
                    .filter(|c| !carrova_manifest::is_unsafe_char(*c))
                    .take(MAX_PEER_MESSAGE)
                    .collect();
                write!(f, "the other machine failed: {clean}")
            }
        }
    }
}

/// How the peer's abort surfaces on this side.
impl From<AbortReason> for crate::error::CoreError {
    fn from(reason: AbortReason) -> Self {
        match reason {
            AbortReason::Cancelled => Self::PeerCancelled,
            other => Self::PeerAborted(other.to_string()),
        }
    }
}

/// Why a LAN import failed, as told to the sender ([`AbortReason::Failed`]).
/// Fixed texts: error details may name receiver paths, which never leave
/// the receiver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ReceiverFailure {
    /// Apply failed; its changes were rolled back.
    ApplyRolledBack,
    /// Apply failed (rollback incomplete or not reached).
    ApplyFailed,
    /// Staging failed locally.
    StageFailed,
    /// An apply was interrupted (crash) and rolled back at resume.
    ApplyInterrupted,
}

impl fmt::Display for ReceiverFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ApplyRolledBack => {
                "applying failed on the receiver; its changes were rolled back"
            }
            Self::ApplyFailed => "applying failed on the receiver",
            Self::StageFailed => "the receiver could not stage the data",
            Self::ApplyInterrupted => {
                "applying was interrupted on the receiver; its changes were rolled back"
            }
        })
    }
}

impl From<ReceiverFailure> for LanMsg {
    fn from(failure: ReceiverFailure) -> Self {
        Self::Abort {
            reason: AbortReason::Failed {
                message: failure.to_string(),
            },
        }
    }
}

#[cfg(test)]
#[path = "../../tests/lan/messages/unit.rs"]
mod tests;
