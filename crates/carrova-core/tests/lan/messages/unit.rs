use super::*;

#[test]
fn receiver_failures_tell_the_sender_fixed_texts() {
    let texts = [
        (
            ReceiverFailure::ApplyRolledBack,
            "applying failed on the receiver; its changes were rolled back",
        ),
        (
            ReceiverFailure::ApplyFailed,
            "applying failed on the receiver",
        ),
        (
            ReceiverFailure::StageFailed,
            "the receiver could not stage the data",
        ),
        (
            ReceiverFailure::ApplyInterrupted,
            "applying was interrupted on the receiver; its changes were rolled back",
        ),
    ];
    for (failure, text) in texts {
        assert_eq!(
            LanMsg::from(failure),
            LanMsg::Abort {
                reason: AbortReason::Failed {
                    message: text.into()
                }
            }
        );
    }
}

#[test]
fn a_peer_cancel_surfaces_as_its_own_error() {
    use crate::error::CoreError;
    assert!(matches!(
        CoreError::from(AbortReason::Cancelled),
        CoreError::PeerCancelled
    ));
    assert!(matches!(
        CoreError::from(AbortReason::DryRun),
        CoreError::PeerAborted(_)
    ));
}

#[test]
fn peer_failure_messages_are_sanitised_and_truncated() {
    let long = AbortReason::Failed {
        message: "x".repeat(MAX_PEER_MESSAGE + 50),
    };
    let shown = long.to_string();
    assert_eq!(
        shown.len(),
        "the other machine failed: ".len() + MAX_PEER_MESSAGE
    );
}
