//! Background-mode commands.

use tauri::State;

use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.setBackground`: `patch` holds only the changed fields.
#[tauri::command]
pub async fn set_background(
    state: State<'_, AppState>,
    patch: serde_json::Value,
) -> Result<(), CommandError> {
    ops::background::set_background(&state, patch).await
}
