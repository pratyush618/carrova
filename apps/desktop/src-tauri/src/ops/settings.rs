//! User settings.

use carrova_view::ipc::Settings;

use crate::command_error::CommandError;
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

/// Save `settings`.
///
/// # Errors
///
/// The settings file cannot be written.
pub async fn save(state: &AppState, settings: Settings) -> Result<(), CommandError> {
    let store = state.settings.clone();
    blocking(move || store.save(&settings)).await
}
