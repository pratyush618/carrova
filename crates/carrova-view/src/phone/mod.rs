//! Presentation models of what a linked phone shows this computer:
//! notifications, SMS, calls, the call-audio setup and the phone's
//! permission steps; and the phone's own per-app notification list.
//! Pure: they return structured data (days back, steps, flags); the
//! front-ends pick the words in their own language.
//!
//! The views carrying a phone's content (text, names, numbers) print
//! only their type's name in `Debug`.

mod apps;
mod calls;
mod messages;
mod notes;
mod setup;

use serde::Deserialize;

pub use apps::{NotifyAppRow, NotifyAppsView, notify_apps_view, phone_apps_view};
pub use calls::{
    CallLogGroup, CallLogView, CallRow, CallWindowView, Caller, call_log_view, call_view,
};
pub use messages::{MessageRow, ThreadRow, ThreadView, ThreadsView, thread_view, threads_view};
pub use notes::{NoteView, NotesView, notes_view, withheld_while_locked};
pub use setup::{CallSetupStep, PermStep, call_audio_view, perm_view};

/// Milliseconds in a day.
const DAY_MS: i64 = 86_400_000;

/// The front-end's clock: now, and how far its time zone is from UTC, so
/// rows group by the user's calendar days ("Today", "Yesterday").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LocalClock {
    /// Now (ms since the Unix epoch).
    pub now_ms: i64,
    /// Local time minus UTC, in minutes (UTC+5:30 is `330`).
    pub utc_offset_min: i32,
}

impl LocalClock {
    /// Calendar days from `when_ms` to now: `0` today, `1` yesterday.
    /// Times ahead of now (another device's clock) count as today.
    // shortcut: one offset for every row, so a row across a DST change can
    // land a day off within an hour of midnight; upgrade with a tz database.
    #[must_use]
    pub fn days_ago(self, when_ms: i64) -> u32 {
        let offset_ms = i64::from(self.utc_offset_min) * 60_000;
        let day = |ms: i64| ms.saturating_add(offset_ms).div_euclid(DAY_MS);
        let days = day(self.now_ms).saturating_sub(day(when_ms)).max(0);
        u32::try_from(days).unwrap_or(u32::MAX)
    }
}
