//! App updates via `tauri-plugin-updater` (minisign-signed, feed =
//! GitHub Releases `latest.json`, see `tauri.conf.json` → `plugins.updater`).
//!
//! Nothing installs on its own: the UI calls `check_update`, shows the
//! version and notes, and only on the user's go-ahead calls
//! `install_update` for that exact version (progress arrives as
//! [`BridgeEvent::Update`](carrova_view::ipc::BridgeEvent::Update); see
//! `install`), then offers `restart_app`.

mod install;

use std::sync::Mutex;
use std::time::Duration;

use carrova_view::ipc::{ErrorCode, UpdateInfo, UpdateStatus};
use carrova_view::update::update_blocker;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};
use tokio::sync::watch;

use crate::call_window::LastCall;
use crate::command_error::CommandError;
use crate::in_flight::{InFlight, Running};
use crate::state::AppState;
use install::Signal;

/// Longest wait for the release feed before the check reports a network error.
const CHECK_TIMEOUT: Duration = Duration::from_secs(10);
/// Ceiling on fetching the installer (~90 MB), so a stalled link fails instead of hanging.
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(30 * 60);

/// The feed's view of `update`, as the UI shows it.
fn update_info(update: &Update) -> UpdateInfo {
    UpdateInfo {
        version: update.version.clone(),
        current_version: update.current_version.clone(),
        notes: update.body.clone(),
        date: update
            .raw_json
            .get("pub_date")
            .and_then(|date| date.as_str())
            .map(str::to_owned),
    }
}

/// The update the user last saw, so the install matches what they approved.
#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

impl std::fmt::Debug for PendingUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PendingUpdate")
    }
}

impl PendingUpdate {
    fn replace(&self, update: Option<Update>) {
        *self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner) = update;
    }

    /// The pending update, if it is `version`. It stays pending until
    /// installed, so a failed or cancelled install can run again.
    fn offered(&self, version: &str) -> Option<Update> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
            .filter(|update| update.version == version)
    }
}

/// Carries the user's Cancel and Install anyway to the install in
/// progress, of which there is at most one.
#[derive(Debug)]
pub struct UpdateControl(watch::Sender<Signal>, InFlight);

impl Default for UpdateControl {
    fn default() -> Self {
        Self(watch::Sender::new(Signal::Go), InFlight::default())
    }
}

impl UpdateControl {
    /// Start an install with a fresh [`Signal::Go`].
    ///
    /// # Errors
    ///
    /// [`ErrorCode::Session`] while another install runs (its signal stays).
    fn begin(&self) -> Result<Running<'_>, CommandError> {
        let running = self.1.start("an update install")?;
        self.0.send_replace(Signal::Go);
        Ok(running)
    }
}

/// `CarrovaPort.checkUpdate`: asks the release feed whether a newer
/// version exists. Never installs.
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<UpdateStatus, CommandError> {
    let mut update = app
        .updater_builder()
        .timeout(CHECK_TIMEOUT)
        .build()?
        .check()
        .await?;
    // The check's timeout carries over to the download; a big file needs longer.
    if let Some(update) = update.as_mut() {
        update.timeout = Some(DOWNLOAD_TIMEOUT);
    }
    let status = update.as_ref().map_or(UpdateStatus::UpToDate, |update| {
        UpdateStatus::Available(update_info(update))
    });
    pending.replace(update);
    Ok(status)
}

/// `CarrovaPort.installUpdate`: downloads, verifies (minisign) and
/// installs `version`, which must be the one the last `check_update`
/// offered. A verified package waits while a move or call runs.
#[tauri::command]
pub async fn install_update(
    app: AppHandle,
    state: State<'_, AppState>,
    pending: State<'_, PendingUpdate>,
    control: State<'_, UpdateControl>,
    version: String,
) -> Result<UpdateStatus, CommandError> {
    let update = pending.offered(&version).ok_or_else(|| {
        CommandError::new(
            ErrorCode::Session,
            format!("version {version} is not on offer; check for updates again"),
        )
    })?;
    let _running = control.begin()?;
    let blocker = || {
        let on_call = app
            .try_state::<LastCall>()
            .is_some_and(|call| call.running().is_some());
        update_blocker(state.sessions.busy(), on_call)
    };
    install::install(&update, &app, blocker, control.0.subscribe()).await?;
    pending.replace(None);
    Ok(UpdateStatus::Installed { version })
}

/// `CarrovaPort.cancelUpdate`: stops the install in progress before it
/// installs anything.
#[tauri::command]
pub async fn cancel_update(control: State<'_, UpdateControl>) -> Result<(), CommandError> {
    control.0.send_replace(Signal::Cancel);
    Ok(())
}

/// `CarrovaPort.installUpdateAnyway`: installs the waiting update now,
/// while a move or call runs.
#[tauri::command]
pub async fn install_update_anyway(control: State<'_, UpdateControl>) -> Result<(), CommandError> {
    control.0.send_replace(Signal::Anyway);
    Ok(())
}

/// `CarrovaPort.restartApp`: restarts Carrova gracefully (after an
/// installed update, when the user chooses).
#[tauri::command]
pub async fn restart_app(app: AppHandle) {
    app.request_restart();
}

#[cfg(test)]
#[path = "../tests/updater/unit.rs"]
mod tests;
