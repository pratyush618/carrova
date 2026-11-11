//! Background mode: what closing the window does, and the tray menu shown
//! while it is closed. Front-ends build the native menu from
//! [`tray_menu`] and dispatch on each item's stable id.

use crate::ipc::AlertSettings;

#[cfg(test)]
#[path = "../tests/tray/unit.rs"]
mod tests;

/// How long the tray's pause item holds pop-ups back, in seconds.
pub const PAUSE_SECS: u64 = 60 * 60;

/// What the tray shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayModel {
    /// A send request waiting for an answer.
    pub request: Option<TrayRequest>,
    /// Pop-ups are held back now ([`alerts_paused`]).
    pub paused: bool,
    /// A move is running in the background.
    pub moving: bool,
}

/// A computer asking to send you files.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayRequest {
    /// The asking computer's name, as it gave it (escape before rendering).
    pub peer: String,
}

/// One tray menu item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrayItem {
    /// Stable id the front-end dispatches on: `moving` (a status line),
    /// `review`, `decline`, `open`, `send`, `pause`, `quit`.
    pub id: &'static str,
    /// What the item says.
    pub label: String,
    /// A separator goes before this item.
    pub separated: bool,
    /// Can be chosen; `false` for a status line.
    pub enabled: bool,
}

/// The tray menu: a running move's status line, the waiting request, then
/// the everyday items.
#[must_use]
pub fn tray_menu(model: &TrayModel) -> Vec<TrayItem> {
    let item = |id, label: &str, separated| TrayItem {
        id,
        label: label.to_owned(),
        separated,
        enabled: true,
    };
    let mut menu = Vec::new();
    if model.moving {
        menu.push(TrayItem {
            enabled: false,
            ..item("moving", "Moving files…", false)
        });
    }
    if let Some(request) = &model.request {
        menu.push(item(
            "review",
            &format!("Review files from {}", request.peer),
            !menu.is_empty(),
        ));
        menu.push(item("decline", "Decline", false));
    }
    let pause = if model.paused {
        "Resume alerts"
    } else {
        "Pause alerts for 1 hour"
    };
    menu.extend([
        item("open", "Open Carrova", !menu.is_empty()),
        item("send", "Send files…", false),
        item("pause", pause, true),
        item("quit", "Quit Carrova", true),
    ]);
    menu
}

/// The tray icon's tooltip.
#[must_use]
pub fn tray_tooltip(model: &TrayModel) -> &'static str {
    if model.moving {
        "Carrova · Moving files"
    } else {
        "Carrova"
    }
}

/// Whether pop-ups are held back at `now` (Unix seconds); a pause that has
/// run out is over.
#[must_use]
pub fn alerts_paused(alerts: &AlertSettings, now: u64) -> bool {
    alerts.paused_until.is_some_and(|until| until > now)
}

/// The `paused_until` the tray's pause item sets: a running pause ends,
/// otherwise pop-ups pause for [`PAUSE_SECS`].
#[must_use]
pub fn toggled_pause(alerts: &AlertSettings, now: u64) -> Option<u64> {
    (!alerts_paused(alerts, now)).then(|| now.saturating_add(PAUSE_SECS))
}

/// What closing the main window does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseAction {
    /// Close the window (its webview is freed) and wait in the tray.
    CloseToTray,
    /// A move is running: hide the window, webview and all, and wait in
    /// the tray; once idle, decide again (it then closes).
    HideToTray,
    /// Keep running without a tray to come back from: minimise instead.
    Minimise,
    /// Quit Carrova.
    Exit,
}

/// What closing the window does, given the user's "Keep running" choice,
/// whether the desktop shows a status tray and whether a move is running
/// (its screen cannot be rebuilt, so the webview stays until it ends).
#[must_use]
pub fn close_action(keep_running: bool, has_tray: bool, busy: bool) -> CloseAction {
    match (keep_running, has_tray, busy) {
        (false, _, _) => CloseAction::Exit,
        (true, false, _) => CloseAction::Minimise,
        (true, true, true) => CloseAction::HideToTray,
        (true, true, false) => CloseAction::CloseToTray,
    }
}
