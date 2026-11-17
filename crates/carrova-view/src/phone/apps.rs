//! The phone's per-app notification list ("Notifications on the laptop"),
//! as the phone itself shows it, and as a linked computer shows it (read
//! only).

use carrova_core::ecosystem::NotifyApps;
use carrova_link::AppEntry;
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
    NotifyAppsView {
        apps: phone_apps_view(&apps.entries(showing)),
        hide_codes: apps.hide_codes(),
        show_text: apps.show_text(),
    }
}

/// A phone's per-app list (`entries`, as it sent them), by name.
#[must_use]
pub fn phone_apps_view(entries: &[AppEntry]) -> Vec<NotifyAppRow> {
    let mut rows: Vec<NotifyAppRow> = entries
        .iter()
        .map(|e| NotifyAppRow {
            id: e.app_id.clone(),
            name: e.app_name.clone(),
            on: e.on,
            private: NotifyApps::is_private(&e.app_id),
        })
        .collect();
    rows.sort_by_cached_key(|r| r.name.to_lowercase());
    rows
}
