//! User settings.

use carrova_view::ipc::{ErrorCode, Settings};
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
/// settings. Background settings change only through
/// [`set_background`](super::background::set_background), which keeps
/// start at sign-in in step with them.
///
/// # Errors
///
/// The patch changes background settings or does not fit, or the settings
/// file cannot be read or written.
pub async fn save(state: &AppState, patch: Value) -> Result<(), CommandError> {
    if patch.get("background").is_some() {
        return Err(CommandError::new(
            ErrorCode::Internal,
            "background settings change through set_background",
        ));
    }
    let store = state.settings.clone();
    blocking(move || {
        let _writing = store.lock();
        store.save(&patched(&store.load()?, patch)?)
    })
    .await
}
