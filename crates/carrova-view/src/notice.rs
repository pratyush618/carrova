//! Which surfaces of an alert fire, by the user's preferences. The bell
//! always keeps the notice; this decides the rest.

use carrova_core::{Notice, NoticeKind};
use carrova_platform::ScreenLock;

use crate::ipc::{AlertChannel, AlertSettings, ErrorCode};
use crate::tray::alerts_paused;

/// Title of every system pop-up.
pub const POPUP_TITLE: &str = "Carrova";

/// What to do, beyond the bell, when a notice arrives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Delivery {
    /// Show a system pop-up.
    pub popup: bool,
    /// Wake the screen for it.
    pub wake: bool,
    /// The pop-up may name devices (otherwise it speaks of "a device").
    pub show_names: bool,
}

/// The delivery of a `kind` notice at `now` (Unix seconds) under `prefs`.
/// `screen_lock` asks the OS; it runs only when a pop-up will show.
#[must_use]
pub fn route(
    kind: NoticeKind,
    prefs: &AlertSettings,
    now: u64,
    screen_lock: impl FnOnce() -> ScreenLock,
) -> Delivery {
    let channel = match kind {
        NoticeKind::SendRequest => prefs.requests,
        NoticeKind::MoveFinished | NoticeKind::MoveFailed => prefs.moves,
        NoticeKind::DeviceNearby => prefs.nearby,
        NoticeKind::UpdateReady => prefs.updates,
    };
    let popup = prefs.system && !alerts_paused(prefs, now) && channel == AlertChannel::PopupAndBell;
    Delivery {
        popup,
        // Waking for an alert that stays silent would only startle.
        wake: popup && prefs.wake_screen && kind == NoticeKind::SendRequest,
        // Fail closed: a screen that may be locked shows no names unless allowed.
        show_names: popup && (prefs.names_on_lock_screen || screen_lock() == ScreenLock::Unlocked),
    }
}

/// The pop-up text of `notice`. `notice.summary` continues the peer's name;
/// without `show_names` the device is not named.
#[must_use]
pub fn popup_text(notice: &Notice, show_names: bool) -> String {
    match (&notice.peer, show_names) {
        (Some(peer), true) => format!("{peer} {}", notice.summary),
        (Some(_), false) => format!("A known device {}", notice.summary),
        (None, _) => notice.summary.clone(),
    }
}

/// What a move that ended with `error` (`None`: it finished) is worth
/// telling the user: the kind and its words. A cancelled move is the
/// user's own doing, so `None`.
#[must_use]
pub fn move_news(error: Option<ErrorCode>) -> Option<(NoticeKind, &'static str)> {
    match error {
        None => Some((NoticeKind::MoveFinished, "The move finished.")),
        Some(ErrorCode::Cancelled) => None,
        Some(_) => Some((NoticeKind::MoveFailed, "The move failed.")),
    }
}
