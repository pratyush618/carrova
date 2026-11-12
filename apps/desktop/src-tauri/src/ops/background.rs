//! Background mode: the Settings rows, what closing the window does and
//! what the tray shows. Policy lives in [`carrova_view::tray`]; this only
//! reads settings and the platform for it.

use std::sync::Arc;

use carrova_core::time::now_unix;
use carrova_view::ipc::{BackgroundSettings, Settings};
use carrova_view::tray::{self, CloseAction, TrayModel};
use serde_json::{Value, json};

use crate::command_error::CommandError;
use crate::settings::patched;
use crate::state::AppState;
use crate::worker::blocking;

/// Apply `patch` (the changed background fields, see [`patched`]) and
/// start at sign-in only while
/// both "Keep running" and "Start at login" are on. Autostart changes first
/// and goes back to how it was if the settings cannot be saved, so a
/// failure changes neither.
///
/// # Errors
///
/// The autostart entry or the settings cannot be written, or the patch
/// does not fit.
pub async fn set_background(state: &AppState, patch: Value) -> Result<(), CommandError> {
    let platform = Arc::clone(&state.platform);
    let store = state.settings.clone();
    blocking(move || {
        let _writing = store.lock();
        let saved = store.load()?;
        let next = patched(&saved, json!({ "background": patch }))?;
        let launcher = platform.launcher()?;
        // What the system does now, which may differ from the saved choice.
        let was_on = platform.autostart_status()?;
        platform.set_autostart(&launcher, starts_at_login(&next.background))?;
        store.save(&next).inspect_err(|_| {
            let undo = platform.set_autostart(&launcher, was_on);
            if let Err(error) = undo {
                tracing::error!(%error, "could not undo the autostart change");
            }
        })
    })
    .await
}

/// Carrova starts at sign-in only while it keeps running in the background.
fn starts_at_login(background: &BackgroundSettings) -> bool {
    background.keep_running && background.start_at_login
}

/// Point an existing autostart entry at this app's current launcher (it
/// moves on updates, e.g. a new AppImage). Blocking; failures are only
/// logged. Holds the settings lock, so a "Start at login" change in
/// progress is never undone by a stale check.
pub fn refresh_autostart(state: &AppState) {
    let _writing = state.settings.lock();
    let platform = state.platform.as_ref();
    let refreshed = platform.autostart_status().and_then(|on| {
        on.then(|| platform.set_autostart(&platform.launcher()?, true))
            .transpose()
    });
    if let Err(error) = refreshed {
        tracing::warn!(%error, "could not refresh the autostart entry");
    }
}

/// What closing the main window does now.
pub fn close_action(state: &AppState) -> CloseAction {
    let background = settings_or_default(state).background;
    tray::close_action(
        background.keep_running,
        state.platform.has_status_tray(),
        state.sessions.busy(),
        background.lean,
    )
}

/// What the tray shows now.
pub fn tray_model(state: &AppState) -> TrayModel {
    model_of(state, &settings_or_default(state), now_unix())
}

/// The tray's pause item: pause pop-ups for an hour, or end a running
/// pause; replies with the tray as it is now.
///
/// # Errors
///
/// The settings cannot be read or written.
pub fn toggle_pause(state: &AppState) -> Result<TrayModel, CommandError> {
    let _writing = state.settings.lock();
    let mut settings = state.settings.load()?;
    let now = now_unix();
    settings.alerts.paused_until = tray::toggled_pause(&settings.alerts, now);
    state.settings.save(&settings)?;
    Ok(model_of(state, &settings, now))
}

fn model_of(state: &AppState, settings: &Settings, now: u64) -> TrayModel {
    TrayModel {
        request: None,
        paused: tray::alerts_paused(&settings.alerts, now),
        moving: state.sessions.busy(),
    }
}

/// The saved settings; the defaults (logged) when they cannot be read, so
/// the window still closes and the tray still shows.
fn settings_or_default(state: &AppState) -> Settings {
    state
        .settings
        .load()
        .inspect_err(|error| tracing::warn!(%error, "settings unreadable; using defaults"))
        .unwrap_or_default()
}
