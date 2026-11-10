//! Engine error type shared by all session modules, the detail types
//! of its variants (`problems`) and its classification for front-ends
//! (`class`).
//!
//! Session-store failures carry the `session store:` prefix in their text
//! (front-ends and session records show these texts verbatim), but each has
//! its own typed variant so callers can match on what went wrong.

use std::path::PathBuf;

use carrova_platform::PlatformError;

use crate::key::ItemKey;
use crate::session::{Role, SessionId, State};
use crate::store::RESUME_KEY_FILE;

mod class;
mod problems;

pub use class::{ErrorClass, ErrorKind, Hint};
pub use problems::{Corruption, EncodeError, JournalProblem, ResumeKeyProblem, SessionProblem};

/// Errors from the session engine. Messages never contain secrets or file
/// contents.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error(transparent)]
    Platform(#[from] PlatformError),
    #[error(transparent)]
    Manifest(#[from] carrova_manifest::ManifestError),
    #[error(transparent)]
    Archive(#[from] carrova_archive::ArchiveError),
    /// A scan failure other than a cancel (that maps to
    /// [`Self::Cancelled`]).
    #[error(transparent)]
    Scan(crate::ScanError),
    /// A transport failure other than a cancel (those map to
    /// [`Self::Cancelled`] / [`Self::PeerCancelled`]).
    #[error(transparent)]
    Transport(carrova_transport::TransportError),
    /// The LAN peer broke Carrova's session protocol.
    #[error("protocol error: {0}")]
    LanProtocol(crate::lan::LanViolation),
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// The requested state change is not allowed from the current state.
    #[error("invalid session transition {from:?} -> {to:?}")]
    InvalidTransition { from: State, to: State },
    /// Selection refers to unknown items or lacks a required confirmation.
    #[error("invalid selection: {0}")]
    Selection(String),
    /// A path from a manifest or remap would escape its semantic root or
    /// pass through a symlink.
    #[error("unsafe target path {path}: {reason}")]
    UnsafePath { path: String, reason: String },
    /// The plan has unresolved blockers (e.g. running app, low space) that
    /// the user has not overridden.
    #[error("plan is blocked: {0}")]
    PlanBlocked(String),
    /// Apply/stage must not run without an approved plan.
    #[error("plan not approved")]
    NotApproved,
    /// Staged data did not match the manifest hash.
    #[error("verification failed for {0}")]
    Verification(String),
    /// Not enough free disk space for staging (receiver). `needed` and
    /// `available` are bytes on the staging filesystem; a LAN transfer
    /// pauses and polls until space frees up.
    #[error("disk full: {needed} bytes needed, {available} available")]
    DiskFull { needed: u64, available: u64 },
    /// A built-in handler (e.g. `gpg-keyring`) failed.
    #[error("handler {handler} failed: {reason}")]
    Handler { handler: String, reason: String },
    /// A captured file kept changing while being read.
    #[error("{0} kept changing during capture")]
    Unstable(String),
    /// The apply journal (rollback point) is missing, unreadable or
    /// cannot be written.
    #[error("session store: {0}")]
    Journal(JournalProblem),
    /// An approved plan names a session id that is not of the id shape.
    #[error("session store: invalid session id in plan")]
    InvalidPlanSession,
    /// A session exists but does not fit the request (wrong role, source
    /// or state).
    #[error("session store: session `{id}` {problem}")]
    Session {
        id: SessionId,
        problem: SessionProblem,
    },
    /// `session.json` names a different session than its directory.
    #[error("session store: session `{id}`: record id does not match")]
    RecordMismatch { id: SessionId },
    /// No session with this id.
    #[error("session store: no session `{0}`")]
    NoSuchSession(SessionId),
    /// A session id that is not of the id shape (never joined into a path).
    #[error("session store: invalid session id `{0}`")]
    InvalidSessionId(String),
    /// `rollback` without an id found no finished import.
    #[error("session store: no finished import to roll back")]
    NothingToRollBack,
    /// A directory the store needs is something else (file, symlink).
    #[error("session store: {} exists and is not a directory", .0.display())]
    NotADirectory(PathBuf),
    /// A stored document exceeds the read limit (corrupt or hostile).
    #[error("session store: {} is too large", .0.display())]
    FileTooLarge(PathBuf),
    /// A file the session needs (e.g. `manifest.cbor`) is missing.
    #[error("session store: session has no {0}")]
    MissingFile(&'static str),
    /// A stored document could not be decoded or fails its checks.
    #[error("session store: {file} is corrupt: {reason}")]
    CorruptFile { file: String, reason: Corruption },
    /// A document could not be encoded for storage.
    #[error("session store: cannot encode {what}: {source}")]
    Encode { what: String, source: EncodeError },
    /// The LAN resume key is unusable.
    #[error("session store: {key} {0}", key = RESUME_KEY_FILE)]
    ResumeKey(ResumeKeyProblem),
    /// The operating system's random number generator failed.
    #[error("session store: random number generator failed")]
    Random,
    /// Every session id tried was taken.
    #[error("session store: could not allocate a session id")]
    IdsExhausted,
    /// The session log could not be opened.
    #[error("session store: cannot open session log {}: {source}", path.display())]
    LogOpen {
        path: PathBuf,
        source: PlatformError,
    },
    /// LAN import: the connection to the sender is closed for good (the
    /// session ended, or a resumed session lost its sender).
    #[error("session store: the LAN session with the sender has ended")]
    LanEnded,
    /// LAN import: the chunk fetch was aborted without a local cause.
    #[error("session store: chunk fetch aborted")]
    FetchAborted,
    /// Apply failed and was rolled back automatically.
    #[error("apply failed ({cause}); changes were rolled back")]
    RolledBack { cause: String },
    /// Apply failed and rollback ALSO failed: manual attention needed.
    #[error("apply failed ({cause}) and rollback failed ({rollback}); see session log")]
    RollbackFailed { cause: String, rollback: String },
    /// The user cancelled the running call ([`crate::Cancel`]). What the
    /// session keeps is documented in [`crate::cancel`].
    #[error("cancelled")]
    Cancelled,
    /// The LAN peer's user cancelled the session. Nothing was applied
    /// here; the session can be resumed once the peer resumes it.
    #[error("the other machine cancelled the session")]
    PeerCancelled,
    /// The LAN peer ended the session early (failed, or — on the
    /// receiver — the sender refused). The message is sanitised.
    #[error("{0}")]
    PeerAborted(String),
    /// The LAN peer stayed unreachable after the connection dropped. The
    /// session is kept and can be resumed.
    #[error("the other machine was unreachable for {waited_secs} s; the session can be resumed")]
    PeerUnreachable { waited_secs: u64 },
    /// LAN transfer: the sender could not supply some items (their files
    /// changed or vanished since capture, or kept failing verification).
    /// Nothing was applied; the receiver is back at its plan (approval
    /// cleared) to re-plan without them or retry.
    #[error(
        "the sender could not supply {}; re-plan without them or retry",
        items.iter().map(ItemKey::as_str).collect::<Vec<_>>().join(", ")
    )]
    TransferIncomplete { items: Vec<ItemKey> },
    /// LAN send: [`crate::lan::MAX_PAIRING_FAILURES`] pairing attempts
    /// failed (wrong codes), so no further code is offered. The send
    /// session ends `Failed`; start a new send to try again.
    #[error(
        "{failures} wrong codes were entered on the other machine; the send was stopped \
         to protect against code guessing. Start a new send to try again"
    )]
    PairingAttemptsExhausted { failures: u32 },
    /// LAN send: after a failed pairing attempt the user chose not to offer
    /// a new code. The send session ends `Failed`.
    #[error("the send was stopped after {failures} wrong code(s); no new code was offered")]
    PairingStopped { failures: u32 },
    /// `resume` without a session id found no interrupted LAN session of
    /// `role`.
    #[error(
        "no interrupted LAN {} to resume",
        match role { Role::Export => "send", Role::Import => "receive" }
    )]
    NothingToResume { role: Role },
    /// Injected failure (feature `failpoints`, tests only).
    #[cfg(feature = "failpoints")]
    #[error("injected failure at {0}")]
    Injected(&'static str),
}

impl From<crate::ScanError> for CoreError {
    /// A cancelled scan is [`Self::Cancelled`]; everything else
    /// [`Self::Scan`].
    fn from(e: crate::ScanError) -> Self {
        match e {
            crate::ScanError::Cancelled => Self::Cancelled,
            e => Self::Scan(e),
        }
    }
}

impl From<carrova_transport::TransportError> for CoreError {
    /// Cancels keep their meaning across layers; everything else is
    /// [`Self::Transport`].
    fn from(e: carrova_transport::TransportError) -> Self {
        use carrova_transport::TransportError as T;
        match e {
            T::Cancelled => Self::Cancelled,
            T::PeerCancelled => Self::PeerCancelled,
            e => Self::Transport(e),
        }
    }
}

impl CoreError {
    /// Whether this means "the LAN connection is gone" (retry or resume),
    /// not "the peer misbehaved".
    #[must_use]
    pub fn is_connection_lost(&self) -> bool {
        matches!(self, Self::Transport(t) if t.is_lost())
    }
}

/// Crate result alias.
pub type Result<T, E = CoreError> = std::result::Result<T, E>;
