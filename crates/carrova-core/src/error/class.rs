//! The one classification of engine errors every front-end branches on:
//! what kind of failure it is, whether the session survives it, and which
//! recovery advice fits ([`CoreError::class`]). Front-ends only phrase it
//! in their own terms (CLI flags, app screens).

use std::error::Error;
use std::io;

use carrova_archive::ArchiveError;
use carrova_crypto::CryptoError;
use carrova_transport::TransportError;

use super::CoreError;

/// What went wrong, as far as the user can act on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorKind {
    /// The user cancelled the running call.
    Cancelled,
    /// The other machine's user cancelled the session.
    PeerCancelled,
    /// The archive did not open: wrong passphrase (or a damaged file;
    /// authentication cannot tell them apart).
    WrongPassphrase,
    /// The typed pairing code is malformed (not a code at all).
    BadCode,
    /// Pairing failed: a wrong code (or a tampered connection), or the
    /// sender ran out of code attempts / stopped offering codes.
    PairingFailed,
    /// No sender answered the code: none found on the network, or (while
    /// connecting) none answered in time.
    CannotConnect,
    /// The selection is unusable (unknown item, missing confirmation).
    InvalidSelection,
    /// The plan has blockers the user has not resolved or overridden.
    Blocked,
    /// Not enough disk space.
    NoSpace,
    /// The network or the other machine failed.
    Network,
    /// Apply failed and was rolled back: nothing changed.
    RolledBack,
    /// Apply failed and rolling back failed too: manual attention needed.
    RollbackFailed,
    /// The session does not exist or does not allow the request.
    Session,
    /// Anything else.
    Internal,
}

/// Recovery advice that fits an error. Front-ends own the wording (and
/// the data they show with it, e.g. [`CoreError::unsupplied_items`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Hint {
    /// The other machine became unreachable; resume once both are back.
    ResumeWhenBack,
    /// The other machine cancelled; resume once it resumes (or dismiss).
    ResumeAfterPeerCancel,
    /// The other machine ended the session.
    PeerEnded,
    /// Nothing was applied: the sender could not supply some items;
    /// deselect them on resume, or retry.
    DeselectUnsupplied,
    /// Check the code the sender shows now, or type its `code@address`.
    CheckCode,
    /// Resolve the blockers, deselect those items, or override.
    ResolveBlockers,
    /// Nothing changed: the apply was rolled back.
    NothingChanged,
    /// Some changes need manual attention (see the session log).
    ManualAttention,
    /// Check the passphrase; if it is right, the file is damaged.
    CheckPassphrase,
}

/// [`CoreError::class`]: everything a front-end needs to react to an
/// error besides its message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ErrorClass {
    /// What went wrong.
    pub kind: ErrorKind,
    /// The session is kept and `resume` can continue it, whichever side
    /// this machine is on.
    pub resumable: bool,
    /// Advice that fits, if any.
    pub hint: Option<Hint>,
}

impl ErrorClass {
    const fn new(kind: ErrorKind) -> Self {
        Self {
            kind,
            resumable: false,
            hint: None,
        }
    }

    const fn hint(mut self, hint: Hint) -> Self {
        self.hint = Some(hint);
        self
    }

    const fn resumable(mut self) -> Self {
        self.resumable = true;
        self
    }
}

impl CoreError {
    /// How front-ends should treat this error (any call but connecting
    /// to a sender: see [`Self::class_when_connecting`]).
    #[must_use]
    pub fn class(&self) -> ErrorClass {
        use ErrorClass as C;
        use ErrorKind as K;
        match self {
            Self::Cancelled => C::new(K::Cancelled),
            Self::PeerCancelled => C::new(K::PeerCancelled)
                .resumable()
                .hint(Hint::ResumeAfterPeerCancel),
            Self::Archive(e) if is_wrong_passphrase(e) => {
                C::new(K::WrongPassphrase).hint(Hint::CheckPassphrase)
            }
            Self::Selection(_) => C::new(K::InvalidSelection),
            Self::PlanBlocked(_) => C::new(K::Blocked).hint(Hint::ResolveBlockers),
            Self::DiskFull { .. } => C::new(K::NoSpace),
            Self::PairingAttemptsExhausted { .. } | Self::PairingStopped { .. } => {
                C::new(K::PairingFailed)
            }
            Self::Transport(t) => transport_class(t),
            Self::PeerUnreachable { .. } => {
                C::new(K::Network).resumable().hint(Hint::ResumeWhenBack)
            }
            Self::PeerAborted(_) => C::new(K::Network).hint(Hint::PeerEnded),
            Self::TransferIncomplete { .. } => C::new(K::Network)
                .resumable()
                .hint(Hint::DeselectUnsupplied),
            Self::LanProtocol(_) => C::new(K::Network),
            Self::RolledBack { .. } => C::new(K::RolledBack).hint(Hint::NothingChanged),
            Self::RollbackFailed { .. } => C::new(K::RollbackFailed).hint(Hint::ManualAttention),
            Self::InvalidTransition { .. }
            | Self::Session { .. }
            | Self::NoSuchSession(_)
            | Self::InvalidSessionId(_)
            | Self::NothingToResume { .. }
            | Self::NothingToRollBack
            | Self::LanEnded
            | Self::NotApproved => C::new(K::Session),
            other if out_of_space(other) => C::new(K::NoSpace),
            _ => C::new(K::Internal),
        }
    }

    /// [`Self::class`] of a failure while pairing with a sender: nobody
    /// answering in time there means no sender could be reached.
    #[must_use]
    pub fn class_when_connecting(&self) -> ErrorClass {
        match self {
            Self::Transport(TransportError::Timeout) => {
                ErrorClass::new(ErrorKind::CannotConnect).hint(Hint::CheckCode)
            }
            other => other.class(),
        }
    }

    /// The items the sender could not supply
    /// ([`CoreError::TransferIncomplete`], [`Hint::DeselectUnsupplied`]).
    #[must_use]
    pub fn unsupplied_items(&self) -> Option<&[crate::ItemKey]> {
        match self {
            Self::TransferIncomplete { items } => Some(items),
            _ => None,
        }
    }
}

/// Class of a transport failure other than a cancel.
fn transport_class(error: &TransportError) -> ErrorClass {
    use ErrorClass as C;
    use ErrorKind as K;
    match error {
        TransportError::PairingFailed | TransportError::Pairing(_) => C::new(K::PairingFailed),
        TransportError::InvalidTicket(_) => C::new(K::BadCode),
        TransportError::NotFound => C::new(K::CannotConnect).hint(Hint::CheckCode),
        _ => C::new(K::Network),
    }
}

/// Whether an archive failed to open because the passphrase is wrong (or
/// the file is damaged: authentication cannot tell them apart).
fn is_wrong_passphrase(error: &ArchiveError) -> bool {
    matches!(error, ArchiveError::Crypto(CryptoError::Decrypt))
}

/// Whether some error in the chain is "no space left on device".
fn out_of_space(error: &(dyn Error + 'static)) -> bool {
    let mut current = Some(error);
    while let Some(e) = current {
        if e.downcast_ref::<io::Error>()
            .is_some_and(|io| io.kind() == io::ErrorKind::StorageFull)
        {
            return true;
        }
        current = e.source();
    }
    false
}
