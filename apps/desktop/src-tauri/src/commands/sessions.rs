//! Commands on any session.

use carrova_core::{RollbackPreview, RollbackReport, SessionId, SessionRecord};
use carrova_view::ipc::{About, SessionOpened};
use tauri::{AppHandle, State};

use super::reply;
use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.cancel`.
#[tauri::command]
pub async fn cancel(state: State<'_, AppState>, session: SessionId) -> Result<(), CommandError> {
    ops::sessions::cancel(&state, &session)
}

/// `CarrovaPort.cancelConnect`.
#[tauri::command]
pub async fn cancel_connect(state: State<'_, AppState>) -> Result<(), CommandError> {
    ops::sessions::cancel_connect(&state);
    Ok(())
}

/// `CarrovaPort.resume`.
#[tauri::command]
pub async fn resume(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
    passphrase: Option<String>,
) -> Result<SessionOpened, CommandError> {
    reply(
        &app,
        ops::sessions::resume(&state, &session, passphrase).await,
    )
}

/// `CarrovaPort.sessions`.
#[tauri::command]
pub async fn sessions(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<Vec<SessionRecord>, CommandError> {
    reply(&app, ops::sessions::list(&state).await)
}

/// `CarrovaPort.rollback`.
#[tauri::command]
pub async fn rollback(
    app: AppHandle,
    state: State<'_, AppState>,
    session: Option<SessionId>,
) -> Result<RollbackReport, CommandError> {
    reply(&app, ops::sessions::rollback(&state, session).await)
}

/// `CarrovaPort.rollbackPreview`.
#[tauri::command]
pub async fn rollback_preview(
    app: AppHandle,
    state: State<'_, AppState>,
    session: Option<SessionId>,
) -> Result<RollbackPreview, CommandError> {
    reply(&app, ops::sessions::rollback_preview(&state, session).await)
}

/// `CarrovaPort.about`.
#[tauri::command]
pub async fn about(state: State<'_, AppState>) -> Result<About, CommandError> {
    Ok(ops::about::about(&state))
}

/// `CarrovaPort.dismiss`.
#[tauri::command]
pub async fn dismiss(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
) -> Result<(), CommandError> {
    reply(&app, ops::sessions::dismiss(&state, &session).await)
}
