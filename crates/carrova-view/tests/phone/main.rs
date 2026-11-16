//! Phone view-models (public API).

#![allow(clippy::unwrap_used, clippy::panic)]

mod apps;
mod calls;
mod messages;
mod notes;
mod setup;

use carrova_view::phone::LocalClock;

/// One day in ms.
const DAY: i64 = 86_400_000;
/// 2026-11-16 00:00 UTC.
const MIDNIGHT: i64 = 20_773 * DAY;
/// 2026-11-16 12:00 UTC.
const NOON: i64 = MIDNIGHT + DAY / 2;

/// A clock at noon UTC on a UTC machine.
fn utc_noon() -> LocalClock {
    LocalClock {
        now_ms: NOON,
        utc_offset_min: 0,
    }
}
