//! Receiving-side commands.

use std::path::PathBuf;

use carrova_core::{Checklist, PlanOptions, SessionId};
use carrova_view::ipc::{PlanReply, SessionOpened};
use tauri::{AppHandle, State};

use super::reply;
use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.openArchive`.
#[tauri::command]
pub async fn open_archive(
    app: AppHandle,
    state: State<'_, AppState>,
    path: PathBuf,
    passphrase: String,
) -> Result<SessionOpened, CommandError> {
    reply(
        &app,
        ops::receive::open_archive(&state, path, passphrase).await,
    )
}

/// `CarrovaPort.connect`.
#[tauri::command]
pub async fn connect(
    app: AppHandle,
    state: State<'_, AppState>,
    code: String,
) -> Result<SessionOpened, CommandError> {
    reply(&app, ops::receive::connect(&state, code).await)
}

/// `CarrovaPort.plan`.
#[tauri::command]
pub async fn plan(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
    options: PlanOptions,
) -> Result<PlanReply, CommandError> {
    reply(&app, ops::receive::plan(&state, &session, options).await)
}

/// `CarrovaPort.planOptions`.
#[tauri::command]
pub async fn plan_options(
    state: State<'_, AppState>,
    session: SessionId,
) -> Result<Option<PlanOptions>, CommandError> {
    ops::receive::plan_options(&state, &session).await
}

/// `CarrovaPort.approve`.
#[tauri::command]
pub async fn approve(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
) -> Result<(), CommandError> {
    reply(&app, ops::receive::approve(&state, &session).await)
}

/// `CarrovaPort.apply`.
#[tauri::command]
pub async fn apply(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
) -> Result<Checklist, CommandError> {
    reply(&app, ops::receive::apply(&state, &session).await)
}

/// `CarrovaPort.setTransferPaused`.
#[tauri::command]
pub async fn set_transfer_paused(
    state: State<'_, AppState>,
    session: SessionId,
    paused: bool,
) -> Result<(), CommandError> {
    ops::receive::set_transfer_paused(&state, &session, paused)
}

/// `CarrovaPort.ignoreFreeSpace`.
#[tauri::command]
pub async fn ignore_free_space(
    state: State<'_, AppState>,
    session: SessionId,
) -> Result<(), CommandError> {
    ops::receive::ignore_free_space(&state, &session)
}

/// `CarrovaPort.endReview`.
#[tauri::command]
pub async fn end_review(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
) -> Result<(), CommandError> {
    reply(&app, ops::receive::end_review(&state, &session).await)
}
