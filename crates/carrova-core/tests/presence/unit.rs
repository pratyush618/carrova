use super::*;

#[test]
fn no_one_sleeps_until_told_otherwise() {
    assert_eq!(next_wake(Visibility::NoOne, 100), None);
}

#[test]
fn known_wakes_to_refresh_its_tags() {
    assert_eq!(next_wake(Visibility::Known, 100), Some(REFRESH));
}

#[test]
fn everyone_wakes_at_its_deadline_or_to_refresh_if_sooner() {
    let soon = Visibility::Everyone { until: 130 };
    assert_eq!(next_wake(soon, 100), Some(Duration::from_secs(30)));
    let later = Visibility::Everyone {
        until: 100 + 2 * REFRESH.as_secs(),
    };
    assert_eq!(next_wake(later, 100), Some(REFRESH));
    assert_eq!(next_wake(soon, 130), Some(REFRESH), "past: known devices");
}

#[test]
fn watched_and_with_nothing_due_it_sets_no_timer() {
    assert_eq!(wait(None, None, Instant::now()), None);
}

#[test]
fn watched_it_wakes_only_when_the_advert_is_due() {
    let now = Instant::now();
    assert_eq!(wait(Some(now + REFRESH), None, now), Some(REFRESH));
    assert_eq!(wait(Some(now), None, now + POLL), Some(Duration::ZERO));
}

#[test]
fn unwatched_it_checks_the_saved_state_every_poll() {
    let now = Instant::now();
    assert_eq!(wait(None, Some(POLL), now), Some(POLL));
    assert_eq!(wait(Some(now + REFRESH), Some(POLL), now), Some(POLL));
    let soon = Duration::from_secs(1);
    assert_eq!(wait(Some(now + soon), Some(POLL), now), Some(soon));
}
