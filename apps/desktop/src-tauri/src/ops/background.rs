//! Background mode: the Settings rows, what closing the window does and
//! what the tray shows. Policy lives in [`carrova_view::tray`]; this only
//! reads settings and the platform for it.

use std::sync::Arc;

use carrova_core::time::now_unix;
use carrova_view::ipc::{BackgroundSettings, Settings};
use carrova_view::tray::{self, CloseAction, TrayModel};

use crate::command_error::CommandError;
use crate::state::AppState;
use crate::worker::blocking;

/// Save `background` and start at sign-in only while both "Keep running"
/// and "Start at login" are on. Autostart changes first, so a failure
/// leaves the saved choice as it was.
///
/// # Errors
///
/// The autostart entry or the settings cannot be written.
pub async fn set_background(
    state: &AppState,
    background: BackgroundSettings,
) -> Result<(), CommandError> {
    let platform = Arc::clone(&state.platform);
    let store = state.settings.clone();
    blocking(move || {
        let _writing = store.lock();
        let on = background.keep_running && background.start_at_login;
        platform.set_autostart(&platform.launcher()?, on)?;
        store.save(&Settings {
            background,
            ..store.load()?
        })
    })
    .await
}

/// Point an existing autostart entry at this app's current launcher (it
/// moves on updates, e.g. a new AppImage). Blocking; failures are only
/// logged.
pub fn refresh_autostart(state: &AppState) {
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
