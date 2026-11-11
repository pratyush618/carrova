//! Notification inbox commands.

use carrova_core::{Notice, NoticeId};
use tauri::State;

use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.notices`.
#[tauri::command]
pub async fn notices(state: State<'_, AppState>) -> Result<Vec<Notice>, CommandError> {
    ops::inbox::list(&state).await
}

/// `CarrovaPort.markNoticesRead` (`ids` null: all of them).
#[tauri::command]
pub async fn mark_notices_read(
    state: State<'_, AppState>,
    ids: Option<Vec<NoticeId>>,
) -> Result<(), CommandError> {
    ops::inbox::mark_read(&state, ids).await
}
