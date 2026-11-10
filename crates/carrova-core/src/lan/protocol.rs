//! The sender's side of the control protocol as a pure state machine
//! ([`SendStep`]), plus manifest splitting and reassembly.

use std::time::Duration;

use carrova_manifest::DEFAULT_MAX_MANIFEST_BYTES;
use carrova_transport::Connection;

use super::{AbortReason, Inventory, LanMsg, MANIFEST_PART_LEN, PlanSummary, ReceiverStage};
use crate::checklist::Checklist;
use crate::error::{CoreError, Result};

/// Where the sender is on the current connection. Every connection starts
/// at [`Self::AwaitInventory`] (fresh pairing) or [`Self::AwaitResume`]
/// (reconnect); the receiver's messages move it on ([`Self::on`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SendStep {
    /// Just paired: the receiver's inventory comes first.
    AwaitInventory,
    /// Reconnected: the receiver says where it is first.
    AwaitResume,
    /// Manifest sent: waiting for the receiver's approved plan (or abort).
    AwaitPlan,
    /// Plan approved: serving fetches until the checklist (or abort).
    Serving,
}

/// What the sender does for one accepted message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SendAction {
    /// Show the inventory, then send the manifest.
    SendManifest(Inventory),
    /// Show the receiver's plan summary.
    ShowPlan(PlanSummary),
    /// The receiver reconnected at `stage`; nothing to send.
    Resumed(ReceiverStage),
    /// Serve chunks until the receiver's fetch ends.
    Serve,
    /// The receiver applied: the session is done.
    Finished(Checklist),
    /// The receiver ended the session early.
    Aborted(AbortReason),
}

/// How the receiver broke the LAN session protocol
/// ([`CoreError::LanProtocol`]). Names message kinds, never contents.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LanViolation {
    /// A message the receiver must not send in the sender's current step.
    #[error("unexpected {message} while {step}")]
    Unexpected {
        /// Kind of the message received.
        message: &'static str,
        /// The sender's step when it arrived.
        step: &'static str,
    },
    /// The announced manifest exceeds [`DEFAULT_MAX_MANIFEST_BYTES`].
    #[error("manifest of {0} bytes exceeds the limit")]
    ManifestTooLarge(u64),
    /// Manifest parts announce different total lengths.
    #[error("manifest parts disagree on its length")]
    ManifestLengthMismatch,
    /// A manifest part overruns the announced length or the part limit.
    #[error("manifest part too long")]
    ManifestPartTooLong,
    /// Something other than a manifest part arrived while receiving it.
    #[error("expected the manifest, got {0}")]
    ExpectedManifest(&'static str),
}

impl From<LanViolation> for CoreError {
    fn from(v: LanViolation) -> Self {
        Self::LanProtocol(v)
    }
}

impl SendStep {
    /// Step name for errors.
    const fn name(self) -> &'static str {
        match self {
            Self::AwaitInventory => "AwaitInventory",
            Self::AwaitResume => "AwaitResume",
            Self::AwaitPlan => "AwaitPlan",
            Self::Serving => "Serving",
        }
    }

    /// Accept `msg` in this step: the next step and what to do, or a
    /// protocol error for a message the receiver must not send now.
    pub(crate) fn on(self, msg: LanMsg) -> Result<(Self, SendAction)> {
        use SendStep::{AwaitInventory, AwaitPlan, AwaitResume, Serving};
        Ok(match (self, msg) {
            (AwaitInventory, LanMsg::Inventory(inv)) => (AwaitPlan, SendAction::SendManifest(inv)),
            (AwaitResume, LanMsg::Resume { stage }) => {
                let next = match stage {
                    ReceiverStage::Planning => AwaitPlan,
                    ReceiverStage::Transferring => Serving,
                };
                (next, SendAction::Resumed(stage))
            }
            (AwaitPlan | Serving, LanMsg::PlanSummary(p)) => (Serving, SendAction::ShowPlan(p)),
            (Serving, LanMsg::Fetch) => (Serving, SendAction::Serve),
            (Serving, LanMsg::Applied(c)) => (Serving, SendAction::Finished(c)),
            (_, LanMsg::Abort { reason }) => (self, SendAction::Aborted(reason)),
            (step, msg) => {
                return Err(LanViolation::Unexpected {
                    message: msg_name(&msg),
                    step: step.name(),
                }
                .into());
            }
        })
    }
}

/// Message kind for errors (never its contents).
fn msg_name(msg: &LanMsg) -> &'static str {
    match msg {
        LanMsg::Inventory(_) => "inventory",
        LanMsg::Manifest { .. } => "manifest",
        LanMsg::PlanSummary(_) => "plan summary",
        LanMsg::Resume { .. } => "resume",
        LanMsg::Fetch => "fetch",
        LanMsg::Applied(_) => "checklist",
        LanMsg::Abort { .. } => "abort",
    }
}

/// Tell the peer this side cancelled ([`AbortReason::Cancelled`], best
/// effort), then close `conn` as cancelled so any call the peer has
/// pending on it ends with [`CoreError::PeerCancelled`] too.
pub(crate) fn farewell_cancelled(mut conn: Connection) {
    let abort = LanMsg::Abort {
        reason: AbortReason::Cancelled,
    };
    if let Err(e) = conn.send(&abort) {
        tracing::debug!(error = %e, "could not tell the peer about the cancel");
    }
    conn.close_cancelled();
}

/// `manifest` (CBOR) as [`LanMsg::Manifest`] parts, in order. An empty
/// manifest is still one (empty) part.
pub(crate) fn manifest_messages(manifest: &[u8]) -> impl Iterator<Item = LanMsg> + '_ {
    let total_len = manifest.len() as u64;
    let parts: Vec<&[u8]> = if manifest.is_empty() {
        vec![manifest]
    } else {
        manifest.chunks(MANIFEST_PART_LEN).collect()
    };
    parts.into_iter().map(move |p| LanMsg::Manifest {
        total_len,
        part: p.to_vec(),
    })
}

/// Receive the manifest parts the sender sends after the inventory and
/// return the whole CBOR. Parts must agree on `total_len`, which must not
/// exceed [`DEFAULT_MAX_MANIFEST_BYTES`]; an abort from the sender is
/// [`CoreError::PeerCancelled`] or [`CoreError::PeerAborted`].
pub(crate) fn recv_manifest(conn: &mut Connection, timeout: Duration) -> Result<Vec<u8>> {
    let mut bytes: Vec<u8> = Vec::new();
    let mut expected: Option<u64> = None;
    loop {
        match conn.recv::<LanMsg>(timeout)? {
            LanMsg::Manifest { total_len, part } => {
                if total_len > DEFAULT_MAX_MANIFEST_BYTES {
                    return Err(LanViolation::ManifestTooLarge(total_len).into());
                }
                if *expected.get_or_insert(total_len) != total_len {
                    return Err(LanViolation::ManifestLengthMismatch.into());
                }
                let len = bytes.len() as u64 + part.len() as u64;
                if len > total_len || part.len() > MANIFEST_PART_LEN {
                    return Err(LanViolation::ManifestPartTooLong.into());
                }
                bytes.extend_from_slice(&part);
                if len == total_len {
                    return Ok(bytes);
                }
            }
            LanMsg::Abort { reason } => return Err(reason.into()),
            other => {
                return Err(LanViolation::ExpectedManifest(msg_name(&other)).into());
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/lan/protocol/unit.rs"]
mod tests;
