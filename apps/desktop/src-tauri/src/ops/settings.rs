//! User settings.

use carrova_view::ipc::Settings;
use serde_json::Value;

use crate::command_error::CommandError;
use crate::settings::patched;
use crate::state::AppState;
use crate::worker::blocking;

/// The saved settings (defaults if none).
///
/// # Errors
///
/// The settings file exists but cannot be read.
pub async fn load(state: &AppState) -> Result<Settings, CommandError> {
    let store = state.settings.clone();
    blocking(move || store.load()).await
}

/// Apply `patch` (the changed fields, see [`patched`]) to the saved
/// settings.
///
/// # Errors
///
/// The settings file cannot be read or written, or the patch does not fit.
pub async fn save(state: &AppState, patch: Value) -> Result<(), CommandError> {
    let store = state.settings.clone();
    blocking(move || {
        let _writing = store.lock();
        store.save(&patched(&store.load()?, patch)?)
    })
    .await
}
