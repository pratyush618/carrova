//! App updates via `tauri-plugin-updater` (minisign-signed, feed =
//! GitHub Releases `latest.json`, see `tauri.conf.json` → `plugins.updater`).
//!
//! Nothing installs on its own: the UI calls `check_update`, shows the
//! version and notes, and only on the user's go-ahead calls
//! `install_update` for that exact version, then offers
//! `restart_app`.

use std::sync::Mutex;

use carrova_view::ipc::{ErrorCode, UpdateInfo, UpdateStatus};
use tauri::{AppHandle, State};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::command_error::CommandError;

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

    /// Takes the pending update if it is `version`.
    fn take_if(&self, version: &str) -> Option<Update> {
        let mut pending = self
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        pending.take_if(|update| update.version == version)
    }
}

/// `CarrovaPort.checkUpdate`: asks the release feed whether a newer
/// version exists. Never installs.
#[tauri::command]
pub async fn check_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<UpdateStatus, CommandError> {
    let update = app.updater()?.check().await?;
    let status = update.as_ref().map_or(UpdateStatus::UpToDate, |update| {
        UpdateStatus::Available(update_info(update))
    });
    pending.replace(update);
    Ok(status)
}

/// `CarrovaPort.installUpdate`: downloads, verifies (minisign) and
/// installs `version`, which must be the one the last `check_update`
/// offered.
#[tauri::command]
pub async fn install_update(
    pending: State<'_, PendingUpdate>,
    version: String,
) -> Result<UpdateStatus, CommandError> {
    let update = pending.take_if(&version).ok_or_else(|| {
        CommandError::new(
            ErrorCode::Session,
            format!("version {version} is not on offer; check for updates again"),
        )
    })?;
    update.download_and_install(|_, _| {}, || {}).await?;
    Ok(UpdateStatus::Installed { version })
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
