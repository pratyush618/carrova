//! Which surfaces of an alert fire, by the user's preferences. The bell
//! always keeps the notice; this decides the rest.

use carrova_core::NoticeKind;
use carrova_platform::ScreenLock;

use crate::ipc::{AlertChannel, AlertSettings};

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
    let paused = prefs.paused_until.is_some_and(|until| now < until);
    let popup = prefs.system && !paused && channel == AlertChannel::PopupAndBell;
    Delivery {
        popup,
        // Waking for an alert that stays silent would only startle.
        wake: popup && prefs.wake_screen && kind == NoticeKind::SendRequest,
        // Fail closed: a screen that may be locked shows no names unless allowed.
        show_names: popup && (prefs.names_on_lock_screen || screen_lock() == ScreenLock::Unlocked),
    }
}
