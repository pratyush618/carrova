use carrova_core::Cancel;

use super::*;

fn armed(announce: bool) -> (Presses, Cancel) {
    let cancel = Cancel::default();
    let mut presses = Presses::new();
    presses.arm(cancel.clone(), announce);
    (presses, cancel)
}

#[test]
fn first_press_cancels_the_running_call() {
    let (mut presses, cancel) = armed(true);
    assert_eq!(presses.press(), Press::Cancelling { announce: true });
    assert!(cancel.is_cancelled());
}

#[test]
fn second_press_force_quits() {
    let (mut presses, _cancel) = armed(true);
    presses.press();
    assert_eq!(presses.press(), Press::ForceQuit);
}

#[test]
fn json_output_cancels_without_announcing() {
    let (mut presses, cancel) = armed(false);
    assert_eq!(presses.press(), Press::Cancelling { announce: false });
    assert!(cancel.is_cancelled());
}

#[test]
fn press_with_nothing_to_cancel_quits_like_the_default() {
    assert_eq!(Presses::new().press(), Press::ForceQuit);
}

#[test]
fn press_after_the_call_returned_quits_and_leaves_the_session_alone() {
    let (mut presses, cancel) = armed(true);
    presses.disarm();
    assert_eq!(presses.press(), Press::ForceQuit);
    assert!(!cancel.is_cancelled());
}

#[test]
fn a_cancelled_call_stays_cancelled_through_a_later_arm() {
    let (mut presses, _first) = armed(true);
    presses.press();
    presses.disarm();
    presses.arm(Cancel::default(), true);
    assert_eq!(presses.press(), Press::ForceQuit);
}

#[test]
fn a_users_cancel_is_not_an_error_to_report() {
    let cancelled = anyhow::Error::new(carrova_core::CoreError::Cancelled).context("resuming");
    assert!(cancelled_by_user(&cancelled));
    assert!(!cancelled_by_user(&anyhow::anyhow!("disk on fire")));
    assert!(!cancelled_by_user(&anyhow::Error::new(
        carrova_core::CoreError::PeerCancelled
    )));
}
