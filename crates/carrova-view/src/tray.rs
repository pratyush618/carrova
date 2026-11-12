//! Background mode: what closing the window does, and the tray menu shown
//! while it is closed. Front-ends build the native menu from
//! [`tray_menu`] and dispatch on each item's stable id.

use carrova_core::presence::EVERYONE_FOR;

use crate::devices::VisibilityChoice;
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
    /// Who may find this computer nearby.
    pub visible_to: VisibilityChoice,
    /// Devices found nearby by the last look; `None` if not looked yet.
    pub nearby_count: Option<u32>,
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
    /// `review`, `decline`, `open`, `send`, `nearby`, `pause`,
    /// `visibility` (a submenu of the [`VISIBILITY_ITEMS`] ids), `quit`.
    pub id: &'static str,
    /// What the item says.
    pub label: String,
    /// A separator goes before this item.
    pub separated: bool,
    /// Can be chosen; `false` for a status line.
    pub enabled: bool,
    /// A check item, ticked or not; `None` for a plain item.
    pub checked: Option<bool>,
    /// A submenu's items; empty for an item.
    pub children: Vec<TrayItem>,
}

/// The "Visible to" submenu: each choice's item id.
pub const VISIBILITY_ITEMS: [(VisibilityChoice, &str); 3] = [
    (VisibilityChoice::NoOne, "visible_no_one"),
    (VisibilityChoice::Known, "visible_known"),
    (VisibilityChoice::Everyone, "visible_everyone"),
];

/// What a "Visible to" item says ("everyone" with its length,
/// [`EVERYONE_FOR`]).
fn visibility_label(choice: VisibilityChoice) -> String {
    match choice {
        VisibilityChoice::NoOne => "No one".to_owned(),
        VisibilityChoice::Known => "Known devices".to_owned(),
        VisibilityChoice::Everyone => {
            format!("Everyone for {} minutes", EVERYONE_FOR.as_secs() / 60)
        }
    }
}

/// The visibility a "Visible to" item id picks, if it is one.
#[must_use]
pub fn tray_visibility(id: &str) -> Option<VisibilityChoice> {
    VISIBILITY_ITEMS
        .iter()
        .find(|(_, item)| *item == id)
        .map(|(choice, _)| *choice)
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
        checked: None,
        children: Vec::new(),
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
    let nearby = match model.nearby_count {
        Some(count) => format!("Nearby · {count} here"),
        None => "Nearby".to_owned(),
    };
    let visibility = TrayItem {
        children: VISIBILITY_ITEMS
            .iter()
            .map(|&(choice, id)| TrayItem {
                checked: Some(choice == model.visible_to),
                ..item(id, &visibility_label(choice), false)
            })
            .collect(),
        ..item("visibility", "Visible to", false)
    };
    menu.extend([
        item("open", "Open Carrova", !menu.is_empty()),
        item("send", "Send files…", false),
        item("nearby", &nearby, false),
        item("pause", pause, true),
        visibility,
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
/// whether the desktop shows a status tray, whether a move is running (its
/// screen cannot be rebuilt, so the webview stays until it ends) and
/// whether to run `lean` (free the webview when idle; otherwise keep the
/// window hidden so reopening is instant).
#[must_use]
pub fn close_action(keep_running: bool, has_tray: bool, busy: bool, lean: bool) -> CloseAction {
    match (keep_running, has_tray) {
        (false, _) => CloseAction::Exit,
        (true, false) => CloseAction::Minimise,
        (true, true) if busy || !lean => CloseAction::HideToTray,
        (true, true) => CloseAction::CloseToTray,
    }
}
