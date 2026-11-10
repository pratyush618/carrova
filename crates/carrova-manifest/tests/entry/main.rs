//! Entry timestamps: Unix-nanosecond conversion.

use std::time::{Duration, UNIX_EPOCH};

use carrova_manifest::{time_from_unix_nanos, unix_nanos};

#[test]
fn nanos_are_signed_around_the_epoch() {
    assert_eq!(unix_nanos(UNIX_EPOCH + Duration::from_nanos(5)), Some(5));
    assert_eq!(unix_nanos(UNIX_EPOCH - Duration::from_nanos(5)), Some(-5));
}

#[test]
fn nanos_decode_back_to_the_same_time() {
    for ns in [0, 5, -5, 1_793_000_000_123_456_789] {
        assert_eq!(unix_nanos(time_from_unix_nanos(ns)), Some(ns));
    }
}
