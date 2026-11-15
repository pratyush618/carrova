//! Linked-device commands: pairing, features and what flows over a link.

use std::path::PathBuf;

use carrova_core::ecosystem::{AskId, ClipSettings};
use carrova_crypto::identity::DeviceId;
use carrova_link::{Feature, FeatureState};
use carrova_view::ecosystem::{ClipHistoryView, LinkedDeviceView};
use tauri::{AppHandle, State};

use super::reply;
use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.linkedDevices`.
#[tauri::command]
pub async fn linked_devices(
    state: State<'_, AppState>,
) -> Result<Vec<LinkedDeviceView>, CommandError> {
    ops::ecosystem::linked_devices(&state).await
}

/// `CarrovaPort.pairInvite`.
#[tauri::command]
pub async fn pair_invite(app: AppHandle, state: State<'_, AppState>) -> Result<(), CommandError> {
    reply(&app, ops::ecosystem::pair_invite(&state).await)
}

/// `CarrovaPort.linkComputer`.
#[tauri::command]
pub async fn link_computer(
    app: AppHandle,
    state: State<'_, AppState>,
    ticket: String,
) -> Result<(), CommandError> {
    reply(&app, ops::ecosystem::link_computer(&state, ticket).await)
}

/// `CarrovaPort.setFeature`.
#[tauri::command]
pub async fn set_feature(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DeviceId,
    feature: Feature,
    feature_state: FeatureState,
) -> Result<(), CommandError> {
    reply(
        &app,
        ops::ecosystem::set_feature(&state, id, feature, feature_state).await,
    )
}

/// `CarrovaPort.answerAsk`.
#[tauri::command]
pub async fn answer_ask(
    state: State<'_, AppState>,
    ask_id: AskId,
    accept: bool,
) -> Result<(), CommandError> {
    ops::ecosystem::answer_ask(&state, ask_id, accept).await
}

/// `CarrovaPort.sendClipboard`.
#[tauri::command]
pub async fn send_clipboard(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DeviceId,
) -> Result<(), CommandError> {
    reply(&app, ops::ecosystem::send_clipboard(&state, id).await)
}

/// `CarrovaPort.sendFiles`.
#[tauri::command]
pub async fn send_files(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DeviceId,
    paths: Vec<PathBuf>,
) -> Result<(), CommandError> {
    reply(&app, ops::ecosystem::send_files(&state, id, paths).await)
}

/// `CarrovaPort.handoffUrl`.
#[tauri::command]
pub async fn handoff_url(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DeviceId,
    url: String,
) -> Result<(), CommandError> {
    reply(&app, ops::ecosystem::handoff_url(&state, id, url).await)
}

/// `CarrovaPort.ring`.
#[tauri::command]
pub async fn ring(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DeviceId,
) -> Result<(), CommandError> {
    reply(&app, ops::ecosystem::ring(&state, id).await)
}

/// `CarrovaPort.clipHistory`.
#[tauri::command]
pub async fn clip_history(state: State<'_, AppState>) -> Result<ClipHistoryView, CommandError> {
    ops::ecosystem::clip_history(&state).await
}

/// `CarrovaPort.clearClipHistory`.
#[tauri::command]
pub async fn clear_clip_history(state: State<'_, AppState>) -> Result<(), CommandError> {
    ops::ecosystem::clear_clip_history(&state).await
}

/// `CarrovaPort.clipSettings`.
#[tauri::command]
pub async fn clip_settings(state: State<'_, AppState>) -> Result<ClipSettings, CommandError> {
    ops::ecosystem::clip_settings(&state).await
}

/// `CarrovaPort.setClipSettings`.
#[tauri::command]
pub async fn set_clip_settings(
    state: State<'_, AppState>,
    settings: ClipSettings,
) -> Result<(), CommandError> {
    ops::ecosystem::set_clip_settings(&state, settings).await
}
