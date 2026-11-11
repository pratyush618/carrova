//! Background-mode commands.

use carrova_view::ipc::BackgroundSettings;
use tauri::State;

use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.setBackground`.
#[tauri::command]
pub async fn set_background(
    state: State<'_, AppState>,
    background: BackgroundSettings,
) -> Result<(), CommandError> {
    ops::background::set_background(&state, background).await
}
