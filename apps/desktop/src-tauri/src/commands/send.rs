//! Sending-side commands.

use std::path::PathBuf;

use carrova_core::{
    ArchiveSummary, KdfStrength, Preset, Selection, SelectionEdit, SendReport, SessionId,
};
use carrova_view::ipc::ScanReply;
use carrova_view::passphrase::PassphraseStrength;
use tauri::{AppHandle, State};

use super::reply;
use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.scan`.
#[tauri::command]
pub async fn scan(app: AppHandle, state: State<'_, AppState>) -> Result<ScanReply, CommandError> {
    reply(&app, ops::send::scan(&state).await)
}

/// `CarrovaPort.suggest`.
#[tauri::command]
pub async fn suggest(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
    preset: Preset,
) -> Result<ScanReply, CommandError> {
    reply(&app, ops::send::suggest(&state, &session, preset).await)
}

/// `CarrovaPort.select`.
#[tauri::command]
pub async fn select(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
    selection: Selection,
    edits: Vec<SelectionEdit>,
) -> Result<ScanReply, CommandError> {
    reply(
        &app,
        ops::send::select(&state, &session, selection, edits).await,
    )
}

/// `CarrovaPort.passphraseStrength`.
#[tauri::command]
pub fn passphrase_strength(passphrase: String) -> PassphraseStrength {
    ops::send::passphrase_strength(passphrase)
}

/// `CarrovaPort.exportArchive`.
#[tauri::command]
pub async fn export_archive(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
    path: PathBuf,
    passphrase: String,
    strength: KdfStrength,
) -> Result<ArchiveSummary, CommandError> {
    let result = ops::send::export_archive(&state, &session, path, passphrase, strength).await;
    reply(&app, result)
}

/// `CarrovaPort.runSend`.
#[tauri::command]
pub async fn run_send(
    app: AppHandle,
    state: State<'_, AppState>,
    session: SessionId,
) -> Result<SendReport, CommandError> {
    reply(&app, ops::send::run_send(&state, &session).await)
}

/// `CarrovaPort.answerPairing`.
#[tauri::command]
pub async fn answer_pairing(
    state: State<'_, AppState>,
    session: SessionId,
    offer_new_code: bool,
) -> Result<(), CommandError> {
    ops::send::answer_pairing(&state, &session, offer_new_code)
}
