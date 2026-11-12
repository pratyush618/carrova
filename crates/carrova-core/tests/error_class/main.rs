//! The one error classification front-ends branch on.

use carrova_archive::ArchiveError;
use carrova_core::{CoreError, ErrorClass, ErrorKind, Hint, ItemKey};
use carrova_crypto::CryptoError;
use carrova_transport::TransportError;

fn class(kind: ErrorKind, resumable: bool, hint: Option<Hint>) -> ErrorClass {
    ErrorClass {
        kind,
        resumable,
        hint,
    }
}

#[test]
fn a_failed_archive_decrypt_is_a_wrong_passphrase() {
    let e = CoreError::Archive(ArchiveError::Crypto(CryptoError::Decrypt));
    assert_eq!(
        e.class(),
        class(
            ErrorKind::WrongPassphrase,
            false,
            Some(Hint::CheckPassphrase)
        )
    );
}

#[test]
fn no_sender_for_a_code_cannot_connect() {
    let e = CoreError::from(TransportError::NotFound);
    assert_eq!(
        e.class(),
        class(ErrorKind::CannotConnect, false, Some(Hint::CheckCode))
    );
}

#[test]
fn a_timeout_only_means_cannot_connect_while_connecting() {
    let e = CoreError::from(TransportError::Timeout);
    assert_eq!(e.class().kind, ErrorKind::Network);
    assert_eq!(
        e.class_when_connecting(),
        class(ErrorKind::CannotConnect, false, Some(Hint::CheckCode))
    );
    assert_eq!(
        CoreError::Cancelled.class_when_connecting(),
        CoreError::Cancelled.class()
    );
}

#[test]
fn pairing_failures_and_bad_codes_are_told_apart() {
    for e in [
        CoreError::from(TransportError::PairingFailed),
        CoreError::PairingAttemptsExhausted { failures: 3 },
        CoreError::PairingStopped { failures: 1 },
    ] {
        assert_eq!(e.class().kind, ErrorKind::PairingFailed, "{e}");
    }
    let bad = CoreError::from(TransportError::InvalidTicket("not a code"));
    assert_eq!(bad.class().kind, ErrorKind::BadCode);
}

#[test]
fn cancels_keep_their_side() {
    assert_eq!(
        CoreError::from(TransportError::Cancelled).class(),
        class(ErrorKind::Cancelled, false, None)
    );
    assert_eq!(
        CoreError::from(TransportError::PeerCancelled).class(),
        class(
            ErrorKind::PeerCancelled,
            true,
            Some(Hint::ResumeAfterPeerCancel)
        )
    );
}

#[test]
fn lan_sessions_that_survive_say_so() {
    assert_eq!(
        CoreError::PeerUnreachable { waited_secs: 600 }.class(),
        class(ErrorKind::Network, true, Some(Hint::ResumeWhenBack))
    );
    let incomplete = CoreError::TransferIncomplete {
        items: vec![ItemKey::from("git/config")],
    };
    assert_eq!(
        incomplete.class(),
        class(ErrorKind::Network, true, Some(Hint::DeselectUnsupplied))
    );
    assert_eq!(
        incomplete.unsupplied_items(),
        Some(&[ItemKey::from("git/config")][..])
    );
    assert_eq!(
        CoreError::PeerAborted("bye".into()).class(),
        class(ErrorKind::Network, false, Some(Hint::PeerEnded))
    );
}

#[test]
fn no_space_is_found_in_the_variant_and_in_io_causes() {
    let full = CoreError::DiskFull {
        needed: 2,
        available: 1,
    };
    assert_eq!(full.class().kind, ErrorKind::NoSpace);
    let io = CoreError::Io(std::io::Error::from(std::io::ErrorKind::StorageFull));
    assert_eq!(io.class().kind, ErrorKind::NoSpace);
    let other = CoreError::Io(std::io::Error::from(std::io::ErrorKind::NotFound));
    assert_eq!(other.class().kind, ErrorKind::Internal);
}

#[test]
fn apply_outcomes_blockers_and_sessions_are_classified() {
    let rolled = CoreError::RolledBack { cause: "x".into() };
    assert_eq!(
        rolled.class(),
        class(ErrorKind::RolledBack, false, Some(Hint::NothingChanged))
    );
    let failed = CoreError::RollbackFailed {
        cause: "x".into(),
        rollback: "y".into(),
    };
    assert_eq!(
        failed.class(),
        class(
            ErrorKind::RollbackFailed,
            false,
            Some(Hint::ManualAttention)
        )
    );
    assert_eq!(
        CoreError::PlanBlocked("b".into()).class(),
        class(ErrorKind::Blocked, false, Some(Hint::ResolveBlockers))
    );
    assert_eq!(CoreError::NotApproved.class().kind, ErrorKind::Session);
    assert_eq!(
        CoreError::NothingToRollBack.class().kind,
        ErrorKind::Session
    );
    assert_eq!(
        CoreError::Selection("x".into()).class().kind,
        ErrorKind::InvalidSelection
    );
}

#[test]
fn known_device_errors_are_their_own_kinds() {
    for e in [
        CoreError::UnknownDevice,
        CoreError::AmbiguousDevice { matches: 2 },
        CoreError::InvalidDeviceName,
    ] {
        assert_eq!(e.class(), class(ErrorKind::KnownDevice, false, None), "{e}");
    }
    let e = CoreError::DeviceKey {
        key: "device.key".into(),
        problem: carrova_core::KeyFileProblem::WrongSize,
    };
    assert_eq!(e.class(), class(ErrorKind::DeviceKey, false, None));
}
