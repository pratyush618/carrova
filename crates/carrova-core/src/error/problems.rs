//! Detail types of [`CoreError`](super::CoreError) variants: why a
//! session, stored document, journal or key file was refused.

use std::fmt;

use crate::session::State;

/// Why a session does not fit a request ([`CoreError::Session`](super::CoreError::Session)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionProblem {
    /// Archive exports are one-shot.
    ExportNotResumable,
    /// Only imports have a rollback point.
    ExportNotRollbackable,
    /// A send was given to the receive side.
    SendNotReceive,
    /// The session's source is not an archive.
    NotArchiveImport,
    /// The session's source is not a LAN sender.
    NotLanImport,
    /// The session is not a LAN send.
    NotLanSend,
    /// The send is not `Transferring` (finished or never started).
    NotInterruptedSend,
    /// The send has no resume secret: no receiver ever paired.
    NeverPaired,
    /// The receive has no resume secret (dismissed or ended).
    NoResumeSecret,
    /// The session is mid-apply.
    Applying,
    /// The session is in `State` and cannot continue.
    Ended(State),
}

impl fmt::Display for SessionProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ExportNotResumable => {
                f.write_str("is an export; exports are not resumable, run the export again")
            }
            Self::ExportNotRollbackable => {
                f.write_str("is an export; only imports can be rolled back")
            }
            Self::SendNotReceive => f.write_str("is a send; resume it as a send"),
            Self::NotArchiveImport => f.write_str("is not an archive import"),
            Self::NotLanImport => f.write_str("is not a LAN import"),
            Self::NotLanSend => f.write_str("is not a LAN send"),
            Self::NotInterruptedSend => f.write_str("is not an interrupted send"),
            Self::NeverPaired => f.write_str("never paired with a receiver; start a new send"),
            Self::NoResumeSecret => {
                f.write_str("has no resume secret (dismissed or ended); start a new receive")
            }
            Self::Applying => f.write_str("is applying; roll it back or resume it first"),
            Self::Ended(state) => write!(f, "is {state:?}; nothing to resume"),
        }
    }
}

/// Why a stored document is corrupt ([`CoreError::CorruptFile`](super::CoreError::CorruptFile)).
#[derive(Debug, thiserror::Error)]
pub enum Corruption {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Cbor(#[from] ciborium::de::Error<std::io::Error>),
    /// Written by an unknown format version.
    #[error("unsupported version {0}")]
    Version(u16),
    /// Belongs to a different file than the one next to it.
    #[error("does not belong to {0}")]
    Mismatch(&'static str),
    /// Refers to data it does not contain.
    #[error("dangling chunk location")]
    Dangling,
}

/// Why a document could not be encoded ([`CoreError::Encode`](super::CoreError::Encode)).
#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Cbor(#[from] ciborium::ser::Error<std::io::Error>),
}

/// What is wrong with an apply journal ([`CoreError::Journal`](super::CoreError::Journal)).
#[derive(Debug, thiserror::Error)]
pub enum JournalProblem {
    /// The session has no journal: nothing was applied.
    #[error("no rollback point for this session")]
    Missing,
    /// Written by a journal version this build cannot undo.
    #[error("unsupported journal version {0}")]
    UnsupportedVersion(u8),
    /// A record could not be encoded.
    #[error("journal encode: {0}")]
    Encode(ciborium::ser::Error<std::io::Error>),
    /// A directory fingerprint could not be encoded.
    #[error("fingerprint encode: {0}")]
    FingerprintEncode(ciborium::ser::Error<std::io::Error>),
    /// An encoded record exceeds the frame limit.
    #[error("journal record too large")]
    RecordTooLarge,
    /// A frame announces a length above the frame limit.
    #[error("journal frame length corrupt")]
    FrameLengthCorrupt,
    /// A complete record does not decode.
    #[error("journal record corrupt: {0}")]
    RecordCorrupt(ciborium::de::Error<std::io::Error>),
}

/// Why a stored secret key file (a session's resume key, the device key
/// or a known device's key) is refused. A key others could read or plant
/// is never used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyFileProblem {
    /// Not exactly the secret's length.
    WrongSize,
    /// The platform reports no permission bits.
    UnknownMode,
    /// Permission bits other than the required ones.
    Mode { found: u32, required: u32 },
    /// Owned by someone else.
    Owner { owner: u32, me: u32 },
}

impl fmt::Display for KeyFileProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::WrongSize => f.write_str("has the wrong size"),
            Self::UnknownMode => f.write_str("has unknown permissions"),
            Self::Mode { found, required } => write!(f, "has mode {found:o}, not {required:o}"),
            Self::Owner { owner, me } => write!(f, "is owned by user {owner}, not by you ({me})"),
        }
    }
}
