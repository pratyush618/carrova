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

/// `CarrovaPort.saveSettings`.
#[tauri::command]
pub async fn save_settings(
    state: State<'_, AppState>,
    settings: Settings,
) -> Result<(), CommandError> {
    ops::settings::save(&state, settings).await
}
