//! A linked phone's notifications, texts and calls on this computer.

use carrova_core::ecosystem::{CallView, PhoneSettings, SyncWindow};
use carrova_crypto::identity::DeviceId;
use carrova_link::Feature;
use carrova_platform::phone::{Contact, Cursor, PhonePerm, SendOutcome, SmsMessage};
use carrova_view::ipc::{CallAction, CallOutcome, NoteAction, NoteOutcome};
use carrova_view::phone::{
    CallLogView, CallSetupStep, CallWindowView, LocalClock, NotesView, NotifyAppRow,
    NotifyAppsView, PermStep, SimChoice, ThreadView, ThreadsView,
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

/// `CarrovaPort.phoneApps`.
#[tauri::command]
pub async fn phone_apps(
    state: State<'_, AppState>,
    id: DeviceId,
) -> Result<Vec<NotifyAppRow>, CommandError> {
    ops::phone::phone_apps(&state, id).await
}

/// `CarrovaPort.remindPhone`.
#[tauri::command]
pub async fn remind_phone(
    app: AppHandle,
    state: State<'_, AppState>,
    id: DeviceId,
    feature: Feature,
) -> Result<bool, CommandError> {
    reply(&app, ops::phone::remind_phone(&state, id, feature).await)
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
    sim: Option<i32>,
) -> Result<SendOutcome, CommandError> {
    reply(
        &app,
        ops::phone::send_sms(&state, id, to, text, client_id, sim).await,
    )
}

/// `CarrovaPort.phoneSims`.
#[tauri::command]
pub async fn phone_sims(
    state: State<'_, AppState>,
    id: DeviceId,
    feature: Feature,
) -> Result<Vec<SimChoice>, CommandError> {
    ops::phone::phone_sims(&state, id, feature).await
}

/// `CarrovaPort.phoneContacts`.
#[tauri::command]
pub async fn phone_contacts(
    state: State<'_, AppState>,
    id: DeviceId,
    query: String,
) -> Result<Vec<Contact>, CommandError> {
    ops::phone::phone_contacts(&state, id, query).await
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

/// `CarrovaPort.callAudioStep`.
#[tauri::command]
pub async fn call_audio_step(state: State<'_, AppState>) -> Result<CallSetupStep, CommandError> {
    ops::phone::call_audio_step(&state).await
}

/// `CarrovaPort.installCallAudio`.
#[tauri::command]
pub async fn install_call_audio(
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), CommandError> {
    reply(&app, ops::phone::install_call_audio(&state).await)
}

/// `CarrovaPort.setCallAudioConfig`.
#[tauri::command]
pub async fn set_call_audio_config(
    app: AppHandle,
    state: State<'_, AppState>,
    on: bool,
) -> Result<(), CommandError> {
    reply(&app, ops::phone::set_call_audio_config(&state, on).await)
}

/// `CarrovaPort.phonePermStep`.
#[tauri::command]
pub async fn phone_perm_step(
    state: State<'_, AppState>,
    perm: PhonePerm,
) -> Result<PermStep, CommandError> {
    ops::phone::phone_perm_step(&state, perm).await
}

/// `CarrovaPort.requestPhonePerm`.
#[tauri::command]
pub async fn request_phone_perm(
    state: State<'_, AppState>,
    perm: PhonePerm,
) -> Result<PermStep, CommandError> {
    ops::phone::request_phone_perm(&state, perm).await
}

/// `CarrovaPort.openPermSettings`.
#[tauri::command]
pub async fn open_perm_settings(
    state: State<'_, AppState>,
    perm: PhonePerm,
) -> Result<(), CommandError> {
    ops::phone::open_perm_settings(&state, perm).await
}

/// `CarrovaPort.notifyApps`.
#[tauri::command]
pub async fn notify_apps(state: State<'_, AppState>) -> Result<NotifyAppsView, CommandError> {
    ops::phone::notify_apps(&state).await
}

/// `CarrovaPort.setNotifyApp`.
#[tauri::command]
pub async fn set_notify_app(
    state: State<'_, AppState>,
    app_id: String,
    on: bool,
) -> Result<(), CommandError> {
    ops::phone::set_notify_app(&state, app_id, on).await
}

/// `CarrovaPort.setNotifyFilters`.
#[tauri::command]
pub async fn set_notify_filters(
    state: State<'_, AppState>,
    hide_codes: bool,
    show_text: bool,
) -> Result<(), CommandError> {
    ops::phone::set_notify_filters(&state, hide_codes, show_text).await
}

/// `CarrovaPort.messagesSync`.
#[tauri::command]
pub async fn messages_sync(state: State<'_, AppState>) -> Result<SyncWindow, CommandError> {
    ops::phone::messages_sync(&state).await
}

/// `CarrovaPort.setMessagesSync`.
#[tauri::command]
pub async fn set_messages_sync(
    state: State<'_, AppState>,
    keep: SyncWindow,
) -> Result<(), CommandError> {
    ops::phone::set_messages_sync(&state, keep).await
}
