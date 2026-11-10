use carrova_core::{SessionId, State};

use super::*;

fn progress(phase: Phase, done: u64) -> Event {
    Event::Progress {
        phase,
        done,
        total: 100,
    }
}

#[test]
fn progress_is_limited_to_one_event_per_interval() {
    let mut throttle = Throttle::default();
    let start = Instant::now();
    assert!(throttle.admit(&progress(Phase::Capture, 1), start));
    assert!(!throttle.admit(
        &progress(Phase::Capture, 2),
        start + Duration::from_millis(10)
    ));
    assert!(throttle.admit(&progress(Phase::Capture, 3), start + PROGRESS_INTERVAL));
}

#[test]
fn a_new_phase_and_a_finished_phase_always_pass() {
    let mut throttle = Throttle::default();
    let now = Instant::now();
    assert!(throttle.admit(&progress(Phase::Capture, 1), now));
    assert!(throttle.admit(&progress(Phase::Capture, 100), now));
    assert!(throttle.admit(&progress(Phase::Archive, 1), now));
}

#[test]
fn state_changes_are_never_dropped() {
    let mut throttle = Throttle::default();
    let now = Instant::now();
    assert!(throttle.admit(&progress(Phase::Stage, 1), now));
    let changed = Event::StateChanged {
        session: SessionId("20261108-101500-a1b2c3".into()),
        from: State::Planned,
        to: State::Transferring,
    };
    assert!(throttle.admit(&changed, now));
    assert!(throttle.admit(&changed, now));
}
