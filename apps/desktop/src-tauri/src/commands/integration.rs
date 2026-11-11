//! File-manager integration commands.

use carrova_platform::FileManagerStatus;
use tauri::State;

use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.fileManagerStatus`.
#[tauri::command]
pub async fn file_manager_status(
    state: State<'_, AppState>,
) -> Result<FileManagerStatus, CommandError> {
    ops::integration::file_manager_status(&state).await
}

/// `CarrovaPort.setFileManagerEntries`.
#[tauri::command]
pub async fn set_file_manager_entries(
    state: State<'_, AppState>,
    on: bool,
) -> Result<FileManagerStatus, CommandError> {
    ops::integration::set_file_manager_entries(&state, on).await
}
