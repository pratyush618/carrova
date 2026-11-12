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
