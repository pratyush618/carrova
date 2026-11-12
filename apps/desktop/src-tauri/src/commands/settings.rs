//! Settings commands.

use carrova_view::ipc::Settings;
use tauri::State;

use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.settings`.
#[tauri::command]
pub async fn settings(state: State<'_, AppState>) -> Result<Settings, CommandError> {
    ops::settings::load(&state).await
}

/// `CarrovaPort.saveSettings`: `patch` holds only the changed fields.
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    patch: serde_json::Value,
) -> Result<(), CommandError> {
    ops::settings::save(&state, patch).await
}
