//! The call windows and the call log.

use carrova_core::ecosystem::CallView;
use carrova_link::HISTORY_PAGE;
use carrova_platform::phone::{AudioSide, CallKind, CallRecord, Cursor};
use serde::Serialize;

use super::LocalClock;

/// Who is on a call. `Debug` prints neither.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Caller {
    /// Their number.
    pub number: String,
    /// Their contact name, resolved on the phone.
    pub name: Option<String>,
}

private_debug!(Caller);

/// What the call windows (ringing, in a call, missed) show.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CallWindowView {
    /// No call shows: the windows close.
    Closed,
    /// An incoming call is ringing on the phone.
    Ringing {
        /// The phone's device id.
        device: String,
        /// Who calls.
        caller: Caller,
        /// Ring here too, not only show it.
        ring_here: bool,
    },
    /// A call is in progress.
    Active {
        /// The phone's device id.
        device: String,
        /// Time since it was answered: `mm:ss`, from an hour `h:mm:ss`;
        /// `None` for a call placed from the phone, whose answer no app
        /// learns ("Calling…").
        timer: Option<String>,
        /// The microphone is muted.
        muted: bool,
        /// Where its audio plays.
        audio: AudioSide,
        /// Offer the keypad: tones go through this computer's call audio,
        /// so only while the audio is here.
        keypad: bool,
    },
    /// A ringing call ended unanswered.
    Missed {
        /// The phone's device id.
        device: String,
        /// Who called.
        caller: Caller,
    },
}

/// Whether the engine's `view` opens the call window: a call ringing
/// here, or one in progress.
#[must_use]
pub fn opens_call_window(view: &CallView) -> bool {
    matches!(
        view,
        CallView::Ringing {
            ring_here: true,
            ..
        } | CallView::Active { .. }
    )
}

/// The window view of the engine's `view` at `now_ms`.
#[must_use]
pub fn call_view(view: &CallView, now_ms: i64) -> CallWindowView {
    match view {
        CallView::None | CallView::Left { .. } => CallWindowView::Closed,
        CallView::Ringing {
            device,
            number,
            name,
            ring_here,
        } => CallWindowView::Ringing {
            device: device.to_hex(),
            caller: caller(number, name.as_ref()),
            ring_here: *ring_here,
        },
        CallView::Active {
            device,
            since_ms,
            muted,
            audio,
            outgoing,
        } => CallWindowView::Active {
            device: device.to_hex(),
            timer: (!outgoing).then(|| timer(now_ms.saturating_sub(*since_ms))),
            muted: *muted,
            audio: *audio,
            keypad: *audio == AudioSide::Laptop,
        },
        CallView::Missed {
            device,
            number,
            name,
        } => CallWindowView::Missed {
            device: device.to_hex(),
            caller: caller(number, name.as_ref()),
        },
    }
}

fn caller(number: &str, name: Option<&String>) -> Caller {
    Caller {
        number: number.to_owned(),
        name: name.cloned(),
    }
}

/// `elapsed_ms` as `mm:ss`, or `h:mm:ss` from an hour; a negative time
/// (the phone's clock ahead of this one) reads `00:00`.
fn timer(elapsed_ms: i64) -> String {
    let secs = elapsed_ms.max(0) / 1000;
    let (h, m, s) = (secs / 3600, secs / 60 % 60, secs % 60);
    if h == 0 {
        format!("{m:02}:{s:02}")
    } else {
        format!("{h}:{m:02}:{s:02}")
    }
}

/// One call in the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CallRow {
    /// Call log row id on the phone.
    pub id: i64,
    /// The other party.
    pub caller: Caller,
    /// When the call started (ms since the Unix epoch).
    pub when_ms: i64,
    /// How long it lasted.
    pub duration_s: u32,
    /// Its direction and outcome.
    pub kind: CallKind,
}

/// The calls of one calendar day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CallLogGroup {
    /// Calendar days back: `0` "Today", `1` "Yesterday", more the date of
    /// its first row.
    pub days_ago: u32,
    /// Newest first.
    pub rows: Vec<CallRow>,
}

/// A page of the call log, by day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CallLogView {
    /// Newest day first. A day split across pages ends one page and
    /// starts the next with the same `days_ago`.
    pub groups: Vec<CallLogGroup>,
    /// Where the next (older) page starts; `None` on the last page.
    pub next: Option<Cursor>,
}

/// The log view of `records` (a page, newest first) at `clock`.
#[must_use]
pub fn call_log_view(records: &[CallRecord], clock: LocalClock) -> CallLogView {
    let days_ago = |r: &CallRecord| clock.days_ago(r.when_ms);
    let groups = records
        .chunk_by(|a, b| days_ago(a) == days_ago(b))
        .filter_map(|day| {
            Some(CallLogGroup {
                days_ago: days_ago(day.first()?),
                rows: day.iter().map(call_row).collect(),
            })
        })
        .collect();
    let next = records
        .last()
        .filter(|_| records.len() >= HISTORY_PAGE)
        .map(|r| Cursor {
            when_ms: r.when_ms,
            id: r.id,
        });
    CallLogView { groups, next }
}

fn call_row(record: &CallRecord) -> CallRow {
    CallRow {
        id: record.id,
        caller: caller(&record.number, record.name.as_ref()),
        when_ms: record.when_ms,
        duration_s: record.duration_s,
        kind: record.kind,
    }
}
