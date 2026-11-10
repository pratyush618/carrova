use std::time::Instant;

use super::*;
use crate::CoreError;
use crate::session::VecSink;

#[test]
fn an_unpaused_transfer_is_not_held_and_announces_nothing() {
    let sink = VecSink::default();
    TransferControl::default()
        .hold_while_paused(&Cancel::default(), &sink)
        .unwrap();
    assert!(sink.events().is_empty());
}

#[test]
fn a_paused_transfer_waits_for_resume_and_announces_both() {
    let control = TransferControl::default();
    control.pause();
    let other = control.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        other.resume();
    });
    let sink = VecSink::default();
    control
        .hold_while_paused(&Cancel::default(), &sink)
        .unwrap();
    assert_eq!(
        sink.events(),
        [
            Event::TransferPaused {
                reason: PauseReason::ByUser
            },
            Event::TransferResumed
        ]
    );
}

#[test]
fn a_cancel_ends_a_pause() {
    let control = TransferControl::default();
    control.pause();
    let cancel = Cancel::default();
    let other = cancel.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        other.cancel();
    });
    let started = Instant::now();
    let held = control.hold_while_paused(&cancel, &VecSink::default());
    assert!(matches!(held, Err(CoreError::Cancelled)));
    assert!(started.elapsed() < Duration::from_secs(5));
}

#[test]
fn ignoring_free_space_is_shared_and_sticky() {
    let control = TransferControl::default();
    let clone = control.clone();
    assert!(!control.ignores_free_space());
    clone.ignore_free_space();
    assert!(control.ignores_free_space());
}

#[test]
fn a_retry_request_is_seen_once_by_any_clone() {
    let control = TransferControl::default();
    assert!(!control.take_retry_now());
    control.clone().retry_now();
    assert!(control.take_retry_now());
    assert!(!control.take_retry_now());
}
