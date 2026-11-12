//! Desktop IPC contract: the owned payloads the Tauri bridge sends to the
//! front-end beyond raw engine types and view-models. Fixed by the main
//! thread (Phase 4 contract); generated into TypeScript with the rest of the
//! bindings.
//!
//! Every engine notification reaches the UI as one [`BridgeEvent`] on the
//! single `carrova://event` channel. Commands return the payloads below or a
//! [`BridgeError`]. Nothing here carries a secret except
//! [`BridgeEvent::PairingCode`], whose code is meant to be shown on screen.

use carrova_core::{ErrorKind, Event, Notice, SessionId};
use serde::{Deserialize, Serialize};

use crate::lan::CodeView;

mod about;
mod defaults;
mod recipes;
mod reply;
mod settings;

pub use about::{About, UpdateInfo, UpdateStatus};
pub use defaults::EngineDefaults;
pub use recipes::{
    CatalogEntry, CatalogItem, RecipeOrigin, RecipeTier, RecipesCheck, RecipesStatus, RecipesUpdate,
};
pub use reply::{PickedReply, PlanReply, ScanReply};
pub use settings::{
    AlertChannel, AlertSettings, BackgroundSettings, MoveDefaults, Settings, Theme,
};

/// Name of the one event channel the bridge emits on.
pub const EVENT_CHANNEL: &str = "carrova://event";

/// Everything the bridge pushes to the front-end.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BridgeEvent {
    /// An engine event of `session`.
    Engine {
        /// Session the event belongs to.
        session: SessionId,
        /// The engine event, unchanged.
        event: Event,
    },
    /// The sender's pairing code is ready (or was renewed): show it.
    PairingCode {
        /// Sending session.
        session: SessionId,
        /// Code to display.
        code: CodeView,
    },
    /// A pairing attempt failed; the user decides whether to offer a new
    /// code. Answer with the `answer_pairing` command.
    ConfirmNewCode {
        /// Sending session.
        session: SessionId,
        /// Failed attempts so far.
        failures: u32,
        /// New codes still allowed this session.
        remaining: u32,
    },
    /// A long-running command of `session` ended (result already returned
    /// to its caller, or failed with `error`).
    TaskEnded {
        /// Session of the task.
        session: SessionId,
        /// Failure, if the task did not succeed.
        error: Option<BridgeError>,
    },
    /// Paths to send arrived from outside (file manager, a second
    /// launch): fetch them with the `take_send_request` command.
    SendRequested,
    /// Something new reached the notification inbox (the bell).
    Notice {
        /// The new, unread notice.
        notice: Notice,
    },
}

/// Why a command failed: a stable `code` for UI logic plus a message and an
/// optional recovery hint, both already user-presentable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BridgeError {
    /// Machine-readable failure class.
    pub code: ErrorCode,
    /// One-line, user-presentable message.
    pub message: String,
    /// What the user can do next, when known.
    pub hint: Option<String>,
}

/// Failure classes the UI branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// The user (or the peer) cancelled.
    Cancelled,
    /// The peer cancelled.
    PeerCancelled,
    /// Wrong archive passphrase.
    WrongPassphrase,
    /// The typed pairing code is malformed (not a code at all).
    BadCode,
    /// Pairing failed: a wrong code (or a tampered connection), or the
    /// sender ran out of code attempts / stopped offering codes.
    PairingFailed,
    /// No sender answered: none found on the network for that code, or
    /// it did not answer in time.
    CannotConnect,
    /// The selection is unusable (unknown item, missing confirmation);
    /// export and send are refused until a usable one is selected.
    InvalidSelection,
    /// The plan has blockers; approve is refused.
    Blocked,
    /// Not enough disk space.
    NoSpace,
    /// Network or peer unreachable.
    Network,
    /// Apply failed and was rolled back: nothing changed.
    RolledBack,
    /// Apply failed and rolling back failed too: manual attention needed.
    RollbackFailed,
    /// No such session (or not in a state that allows the command).
    Session,
    /// Recipe bundle problem (verification, download).
    Recipes,
    /// A known device could not be picked or renamed.
    KnownDevice,
    /// A stored device key is refused.
    DeviceKey,
    /// Anything else.
    Internal,
}

impl From<ErrorKind> for ErrorCode {
    /// The wire code of an engine error class ([`carrova_core::CoreError::class`]).
    fn from(kind: ErrorKind) -> Self {
        match kind {
            ErrorKind::Cancelled => Self::Cancelled,
            ErrorKind::PeerCancelled => Self::PeerCancelled,
            ErrorKind::WrongPassphrase => Self::WrongPassphrase,
            ErrorKind::BadCode => Self::BadCode,
            ErrorKind::PairingFailed => Self::PairingFailed,
            ErrorKind::CannotConnect => Self::CannotConnect,
            ErrorKind::InvalidSelection => Self::InvalidSelection,
            ErrorKind::Blocked => Self::Blocked,
            ErrorKind::NoSpace => Self::NoSpace,
            ErrorKind::Network => Self::Network,
            ErrorKind::RolledBack => Self::RolledBack,
            ErrorKind::RollbackFailed => Self::RollbackFailed,
            ErrorKind::Session => Self::Session,
            ErrorKind::KnownDevice => Self::KnownDevice,
            ErrorKind::DeviceKey => Self::DeviceKey,
            ErrorKind::Internal => Self::Internal,
        }
    }
}

/// Returned when a command opens a session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SessionOpened {
    /// The new session.
    pub session: SessionId,
}

/// `scan`, `suggest` and `select` result: a selection of the export
/// session and its view.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScanResult<'a> {
    /// The export session the scan opened.
    pub session: SessionId,
    /// Full scan report (advanced tree, sizes, flags).
    pub report: &'a carrova_core::ScanReport,
    /// The selection: the suggestion, or the result of the user's edits
    /// (`select` applies them by the engine's rules).
    pub selection: &'a carrova_core::Selection,
    /// Presentation of `selection` against `report`.
    pub view: crate::selection::SelectionView<'a>,
}

/// `pick` result: the session's selection with the picks added, and what
/// happened to each picked path.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Picked<'a> {
    /// The selection after the picks, and its view.
    pub scan: ScanResult<'a>,
    /// One outcome per picked path, in order.
    pub outcomes: &'a [carrova_core::PickOutcome],
}

/// `plan` result on the receiver: everything the review screens show.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlanReview<'a> {
    /// The plan itself (for the advanced sheet: blockers, warnings, actions).
    pub plan: &'a carrova_core::Plan,
    /// Grouped presentation of the plan (always [`crate::Detail::Full`]; the
    /// UI collapses).
    pub view: crate::plan::PlanView<'a>,
    /// Free-space check per filesystem.
    pub space: crate::plan::SpaceView,
    /// Every item the source offered, picked or not, with sensitivity.
    pub offered: Vec<crate::offer::OfferedItem<'a>>,
    /// Whether the user still has to approve the plan (a resumed
    /// session's plan approved earlier with the same options does not).
    pub approval: carrova_core::PlanApproval,
    /// The computer the data comes from; `None` when the source's
    /// manifest can no longer be read.
    pub source: Option<SourceDevice<'a>>,
}

/// The computer a transfer comes from, as its manifest describes it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SourceDevice<'a> {
    /// Its OS family.
    pub os: carrova_platform::OsKind,
    /// Its device name (validated displayable text), if it sent one
    /// (older senders do not).
    pub device_name: Option<&'a str>,
}

impl<'a> ScanResult<'a> {
    /// The scan result of export session `session`: `selection` (suggested
    /// or edited) presented against `report`.
    #[must_use]
    pub fn new(
        session: SessionId,
        report: &'a carrova_core::ScanReport,
        selection: &'a carrova_core::Selection,
    ) -> Self {
        Self {
            session,
            report,
            selection,
            view: crate::selection::selection_view(selection, report),
        }
    }
}

impl<'a> PlanReview<'a> {
    /// Everything the review screens show about `plan`: its full view, the
    /// engine's space needs, its `approval` and, when the source's
    /// `manifest` is still readable, every offered item (empty otherwise).
    /// Front-ends get it from [`crate::review::Prepared::review`].
    #[must_use]
    pub fn new(
        plan: &'a carrova_core::Plan,
        manifest: Option<&'a carrova_core::Manifest>,
        needs: &[carrova_core::plan::SpaceNeed],
        approval: carrova_core::PlanApproval,
    ) -> Self {
        Self {
            plan,
            view: crate::plan::plan_view(plan, crate::Detail::Full),
            space: crate::plan::space_view(plan, needs),
            offered: manifest.map_or_else(Vec::new, |m| crate::offer::offered_items(m, plan)),
            approval,
            source: manifest.map(|m| SourceDevice {
                os: m.source.os,
                device_name: m.source.device_name.as_deref(),
            }),
        }
    }
}
