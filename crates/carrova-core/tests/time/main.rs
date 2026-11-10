//! The engine's one clock and calendar.

use carrova_core::time::{civil_from_days, date, timestamp, utc_date};

#[test]
fn timestamps_are_utc_and_sortable() {
    assert_eq!(timestamp(0), "19700101-000000");
    assert_eq!(timestamp(1_793_000_000), "20261026-073320");
}

#[test]
fn calendar_handles_leap_days_and_pre_epoch() {
    assert_eq!(civil_from_days(0), (1970, 1, 1));
    assert_eq!(civil_from_days(11_016), (2000, 2, 29));
    assert_eq!(civil_from_days(-1), (1969, 12, 31));
}

#[test]
fn dates_shown_to_people_are_iso_formatted() {
    assert_eq!(utc_date(0), "1970-01-01");
    assert_eq!(utc_date(951_782_400), "2000-02-29");
    assert_eq!(utc_date(1_800_000_000), "2027-01-15");
    assert_eq!(date(1_800_000_000), "20270115");
}
