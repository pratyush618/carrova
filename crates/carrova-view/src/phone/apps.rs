//! The phone's per-app notification list ("Notifications on the laptop"),
//! as the phone itself shows it.

use carrova_core::ecosystem::NotifyApps;
use carrova_platform::phone::Note;
use serde::Serialize;

/// One app that may post notifications. `Debug` prints no content.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NotifyAppRow {
    /// Package id: what [`NotifyApps::set`] takes.
    pub id: String,
    /// The app's display name (its package id when no note named it).
    pub name: String,
    /// Whether its notifications reach the linked computers.
    pub on: bool,
    /// A banking, payment, password or authenticator app: off unless the
    /// user turned it on.
    pub private: bool,
}

private_debug!(NotifyAppRow);

/// The list and its two content filters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NotifyAppsView {
    /// By name.
    pub apps: Vec<NotifyAppRow>,
    /// Withhold notifications carrying a one-time code.
    pub hide_codes: bool,
    /// Send message text (off: the app name and title only).
    pub show_text: bool,
}

/// The apps of the notes `showing` on the phone and every app the user
/// chose for, set as `apps` says, by name.
#[must_use]
pub fn notify_apps_view(apps: &NotifyApps, showing: &[Note]) -> NotifyAppsView {
    let mut rows: Vec<NotifyAppRow> = Vec::new();
    let named = showing
        .iter()
        .map(|n| (n.app_id.as_str(), n.app_name.as_str()));
    let chosen = apps.chosen().map(|id| (id, id));
    for (id, name) in named.chain(chosen) {
        if !id.is_empty() && rows.iter().all(|r| r.id != id) {
            rows.push(NotifyAppRow {
                id: id.to_owned(),
                name: name.to_owned(),
                on: apps.allowed(id),
                private: NotifyApps::is_private(id),
            });
        }
    }
    rows.sort_by_cached_key(|r| r.name.to_lowercase());
    NotifyAppsView {
        apps: rows,
        hide_codes: apps.hide_codes(),
        show_text: apps.show_text(),
    }
}
