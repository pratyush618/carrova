use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Mutex;

use carrova_crypto::identity::DeviceId;
use serde::{Deserialize, Serialize};

use crate::checklist::Checklist;
use crate::ecosystem::{AskId, AskKind, ClipSkip, FileFailure};
use crate::key::ItemKey;
use crate::lan::{Inventory, PlanSummary};
use crate::session::{Phase, SessionId, State};

/// What a rollback did with one path ([`Event::RollbackStep`]; the same
/// split as the lists of [`crate::RollbackReport`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RollbackOutcome {
    /// The pre-import version is back.
    Restored,
    /// Something the import created was deleted.
    Removed,
    /// Changed since the import: moved to the session's `rollback-trash/`
    /// instead of being deleted.
    MovedAside,
    /// Could not be undone; needs manual attention.
    Failed,
}

/// Typed progress/notification stream rendered by front-ends.
/// Never carries file contents, secrets, passphrases or pairing codes —
/// paths only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum Event {
    StateChanged {
        session: SessionId,
        from: State,
        to: State,
    },
    /// Byte progress of the current phase.
    Progress {
        phase: Phase,
        done: u64,
        total: u64,
    },
    ItemCaptured {
        item: ItemKey,
        files: u64,
        bytes: u64,
    },
    /// A file changed while being captured and was re-read.
    Recaptured {
        path: String,
        attempt: u32,
    },
    ConflictFound {
        item: ItemKey,
        target: String,
    },
    ActionApplied {
        item: ItemKey,
        target: String,
        action: String,
    },
    /// One path handled by a rollback, and how.
    RollbackStep {
        target: String,
        outcome: RollbackOutcome,
    },
    Warning {
        message: String,
    },
    Finished {
        checklist: Checklist,
    },
    /// Paired with (or reconnected to) a LAN peer.
    PeerConnected {
        peer: SocketAddr,
    },
    /// The LAN connection dropped; `retrying` when the engine reconnects
    /// automatically (the session stays resumable either way).
    PeerLost {
        retrying: bool,
    },
    /// Sender: the receiver's inventory arrived.
    InventoryReceived(Inventory),
    /// Sender: the receiver's plan summary arrived.
    PlanSummaryReceived(PlanSummary),
    /// Receiver: chunk transfer paused until the condition clears.
    TransferPaused {
        reason: PauseReason,
    },
    /// Receiver: chunk transfer continues after a pause.
    TransferResumed,
    /// A pairing attempt failed (wrong code or tampered connection). The
    /// sender burns the code and shows a fresh one.
    PairingFailed,
    /// Sender: a receiver started the pairing handshake with the offered
    /// code; [`Event::PeerConnected`] or [`Event::PairingFailed`] follows.
    PairingInProgress,
    /// After a successful code-paired move, the peer is remembered (again,
    /// if it was known: the new pairing's secret replaces the old). Its
    /// local name and display fingerprint (`7F3A·19C2`), never its full
    /// key.
    DeviceRemembered {
        name: String,
        fingerprint: String,
    },
    /// A linked device connected (not repeated when a newer connection
    /// replaces a live one).
    LinkUp {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
    },
    /// The link to a device closed; `retrying` while this side redials.
    LinkDown {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        retrying: bool,
    },
    /// A linked device's policy arrived or changed.
    PolicyChanged {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
    },
    /// A device offers something that needs the user's answer
    /// ([`crate::ecosystem::Supervisor::answer`]).
    AskReceived {
        ask: AskId,
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        kind: AskKind,
    },
    /// An ask went unanswered (or its link closed); answering it now fails.
    AskExpired {
        ask: AskId,
    },
    /// A device's clipboard arrived and replaced this one's; `note` if the
    /// user wants to be told ([`crate::ecosystem::ClipSettings::show_note`]).
    ClipReceived {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        note: bool,
    },
    /// This clipboard reached a device.
    ClipSent {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
    },
    /// The clipboard was not shared.
    ClipSkipped {
        reason: ClipSkip,
    },
    /// Bytes of file `id` sent to or received from a device so far.
    FileProgress {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        id: u64,
        done: u64,
        total: u64,
    },
    /// File `id` from a device was saved at `path` (in Downloads).
    FileSaved {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        id: u64,
        path: PathBuf,
    },
    /// File `id` this device offered was sent: the other device saved it.
    FileSent {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        id: u64,
    },
    /// File `id` to or from a device did not arrive; `retry` if it can be
    /// sent again ([`crate::ecosystem::Supervisor::retry_file`]).
    FileFailed {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
        id: u64,
        reason: FileFailure,
        retry: bool,
    },
    /// A device's handed-off link or file was opened here.
    HandoffOpened {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
    },
    /// A device made this one ring.
    RingStarted {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
    },
    /// A device stopped this one ringing.
    RingStopped {
        #[cfg_attr(feature = "ts", ts(type = "string"))]
        device: DeviceId,
    },
}

/// Why a LAN transfer paused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PauseReason {
    /// Not enough free space in the staging filesystem.
    LowDiskSpace {
        /// Where space is short: Carrova's state directory (the staging
        /// area's filesystem), safe to display.
        fs: String,
        /// Free bytes now.
        available: u64,
        /// Bytes needed to continue.
        needed: u64,
        /// Mount point of that filesystem, if the platform knows it.
        #[serde(default)]
        disk: Option<String>,
    },
    /// The user paused the transfer
    /// ([`crate::TransferControl::pause`]).
    ByUser,
}

/// Receives engine events. Must be cheap and non-blocking.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: Event);
}

/// Discards events.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl EventSink for NullSink {
    fn emit(&self, _event: Event) {}
}

/// Collects events (tests, JSON output).
#[derive(Debug, Default)]
pub struct VecSink(pub Mutex<Vec<Event>>);

impl VecSink {
    /// Snapshot of collected events.
    #[must_use]
    pub fn events(&self) -> Vec<Event> {
        self.0.lock().map(|v| v.clone()).unwrap_or_default()
    }
}

impl EventSink for VecSink {
    fn emit(&self, event: Event) {
        if let Ok(mut v) = self.0.lock() {
            v.push(event);
        }
    }
}
