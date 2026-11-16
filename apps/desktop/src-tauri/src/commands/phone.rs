//! A linked phone's notifications, texts and calls on this computer.

use carrova_core::ecosystem::{CallView, PhoneSettings};
use carrova_crypto::identity::DeviceId;
use carrova_platform::phone::{Cursor, SendOutcome, SmsMessage};
use carrova_view::ipc::{CallAction, CallOutcome, NoteAction, NoteOutcome};
use carrova_view::phone::{
    CallLogView, CallWindowView, LocalClock, NotesView, ThreadView, ThreadsView,
};
use tauri::{AppHandle, State};

use super::reply;
use crate::command_error::CommandError;
use crate::ops;
use crate::state::AppState;

/// `CarrovaPort.phoneNotes`.
#[tauri::command]
pub async fn phone_notes(
    state: State<'_, AppState>,
    id: DeviceId,
) -> Result<NotesView, CommandError> {
    ops::phone::phone_notes(&state, id).await
}

/// `CarrovaPort.phoneNoteAction`.
#[tauri::command]
pub async fn phone_note_action(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DeviceId,
    action: NoteAction,
) -> Result<NoteOutcome, CommandError> {
    reply(
        &app,
        ops::phone::phone_note_action(&state, id, action).await,
    )
}

/// `CarrovaPort.phoneThreads`.
#[tauri::command]
pub async fn phone_threads(
    state: State<'_, AppState>,
    id: DeviceId,
    before: Option<Cursor>,
    incoming: Vec<SmsMessage>,
    clock: LocalClock,
) -> Result<ThreadsView, CommandError> {
    ops::phone::phone_threads(&state, id, before, incoming, clock).await
}

/// `CarrovaPort.phoneThread`.
#[tauri::command]
pub async fn phone_thread(
    state: State<'_, AppState>,
    id: DeviceId,
    thread: i64,
    before: Option<Cursor>,
) -> Result<ThreadView, CommandError> {
    ops::phone::phone_thread(&state, id, thread, before).await
}

/// `CarrovaPort.sendSms`.
#[tauri::command]
pub async fn send_sms(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DeviceId,
    to: String,
    text: String,
    client_id: u64,
) -> Result<SendOutcome, CommandError> {
    reply(
        &app,
        ops::phone::send_sms(&state, id, to, text, client_id).await,
    )
}

/// `CarrovaPort.phoneCall`.
#[tauri::command]
pub async fn phone_call(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DeviceId,
    action: CallAction,
) -> Result<CallOutcome, CommandError> {
    reply(&app, ops::phone::phone_call(&state, id, action).await)
}

/// `CarrovaPort.callHistory`.
#[tauri::command]
pub async fn call_history(
    state: State<'_, AppState>,
    id: DeviceId,
    before: Option<Cursor>,
    clock: LocalClock,
) -> Result<CallLogView, CommandError> {
    ops::phone::call_history(&state, id, before, clock).await
}

/// `CarrovaPort.callWindow`.
#[tauri::command]
pub async fn call_window(view: CallView, now_ms: i64) -> CallWindowView {
    ops::phone::call_window(&view, now_ms)
}

/// `CarrovaPort.phoneSettings`.
#[tauri::command]
pub async fn phone_settings(state: State<'_, AppState>) -> Result<PhoneSettings, CommandError> {
    ops::phone::phone_settings(&state).await
}

/// `CarrovaPort.setPhoneSettings`.
#[tauri::command]
pub async fn set_phone_settings(
    state: State<'_, AppState>,
    settings: PhoneSettings,
) -> Result<(), CommandError> {
    ops::phone::set_phone_settings(&state, settings).await
}
