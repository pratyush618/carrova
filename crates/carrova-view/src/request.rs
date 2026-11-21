//! Move requests between trusted computers (ADR 0004, Proposed): what a
//! request shows before its receiver opens Review, and how it ends. Before
//! Review a request carries counts only, never app names. Pure: names are
//! this computer's own records, and front-ends escape them.

use carrova_platform::ScreenLock;
use serde::{Deserialize, Serialize};

/// How long a request waits for an answer, in seconds.
pub const REQUEST_SECS: u64 = 120;

/// What a move request tells its receiver before Review: counts only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RequestSummary {
    /// Apps in the move.
    pub apps: u32,
    /// Bytes in the move.
    pub bytes: u64,
    /// The move holds keys or tokens (each still asks on its own).
    pub has_sensitive: bool,
}

/// A trusted computer's request to send this one a move, waiting for an
/// answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MoveRequestView {
    /// Handle the answer names; unique while the app runs.
    pub id: u32,
    /// The asking computer's device id (full key, hex).
    pub device: String,
    /// Its name as this computer remembers it.
    pub name: String,
    /// What it would send.
    pub summary: RequestSummary,
    /// When it expires unanswered (Unix seconds).
    pub expires_at: u64,
}

/// Why a request ended before it was answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RequestEnd {
    /// The sender cancelled it.
    Cancelled,
    /// Nobody answered within [`REQUEST_SECS`].
    Expired,
    /// The same computer sent a newer one (one pending request per device).
    Superseded,
    /// Either side left the network.
    Lost,
}

/// The receiver's answer to a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RequestAnswer {
    /// Open the normal receive review (dry run, approve, keys ask).
    Review,
    /// Say no; nothing is sent.
    Decline,
}

/// A request's system pop-up.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestPopup {
    /// First line.
    pub title: String,
    /// What follows it.
    pub body: String,
}

/// `12 apps · 340.0 MB · includes keys`.
#[must_use]
pub fn summary_text(summary: &RequestSummary) -> String {
    let apps = match summary.apps {
        1 => "1 app".to_owned(),
        n => format!("{n} apps"),
    };
    let size = human_size(summary.bytes);
    if summary.has_sensitive {
        format!("{apps} · {size} · includes keys")
    } else {
        format!("{apps} · {size}")
    }
}

/// The pop-up for `request` at `now` (Unix seconds). A screen that is
/// locked, or may be, shows no counts and names the computer only when
/// `names_on_lock_screen` allows it; by default it is "A trusted computer".
#[must_use]
pub fn request_popup(
    request: &MoveRequestView,
    now: u64,
    lock: ScreenLock,
    names_on_lock_screen: bool,
) -> RequestPopup {
    let left = clock(request.expires_at.saturating_sub(now));
    if lock == ScreenLock::Unlocked {
        return RequestPopup {
            title: format!("{} wants to send you its setup", request.name),
            body: format!(
                "{}. Nothing changes until you review it. {left} left.",
                summary_text(&request.summary)
            ),
        };
    }
    let who = if names_on_lock_screen {
        request.name.as_str()
    } else {
        "A trusted computer"
    };
    RequestPopup {
        title: format!("{who}: 1 request"),
        body: format!("Unlock to see it. Details stay hidden while locked. It expires in {left}."),
    }
}

/// `102` → `1:42`.
fn clock(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// `1500` → `1.5 kB`: SI units, one decimal, as the CLI and the desktop
/// app print sizes.
// shortcut: copies the CLI's `render::text::human_size`; share one when the CLI may change.
fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["kB", "MB", "GB", "TB", "PB", "EB"];
    /// Smallest value that prints as `1000.0` with one decimal.
    const ROLLS_OVER: f64 = 999.95;
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    #[expect(clippy::cast_precision_loss, reason = "display only")]
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= ROLLS_OVER && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}
