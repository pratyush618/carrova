//! A linked phone on this computer: its notifications, texts and calls,
//! and the user's settings for showing them. Each call goes to the link
//! supervisor (as in [`super::ecosystem`]); replies leave as the shared
//! phone view-models.

use carrova_core::ecosystem::{CallView, PhoneSettings, SupervisorHandle};
use carrova_crypto::identity::DeviceId;
use carrova_link::{CallCommand, CallReply, SmsReply, SmsRequest};
use carrova_platform::phone::{Cursor, SendOutcome, SmsMessage};
use carrova_view::ipc::{CallAction, CallOutcome, ErrorCode, NoteAction, NoteOutcome};
use carrova_view::phone::{
    CallLogView, CallWindowView, LocalClock, NotesView, ThreadView, ThreadsView, call_log_view,
    call_view, notes_view, thread_view, threads_view, withheld_while_locked,
};

use super::ecosystem::with_link;
use crate::command_error::CommandError;
use crate::state::AppState;

/// The phone answered a request with a reply of another kind.
fn unexpected_reply() -> CommandError {
    CommandError::new(ErrorCode::Internal, "the phone sent an unexpected reply")
}

/// The notifications `device` shows.
///
/// # Errors
///
/// Unknown device, or the supervisor is not ready or cannot be reached.
pub async fn phone_notes(state: &AppState, device: DeviceId) -> Result<NotesView, CommandError> {
    let notes = with_link(state, move |link| link.notes(&device)).await?;
    Ok(notes_view(&notes, withheld_while_locked(&notes)))
}

/// Reply to, dismiss or open one of `device`'s notifications.
///
/// # Errors
///
/// Not linked, notifications off for it, or the link failed.
pub async fn phone_note_action(
    state: &AppState,
    device: DeviceId,
    action: NoteAction,
) -> Result<NoteOutcome, CommandError> {
    with_link(state, move |link| {
        link.note_action(&device, action.clone().into())
    })
    .await
    .map(NoteOutcome::from)
}

/// A page of `device`'s conversations after `before` (`None`: the
/// newest), with the provisional texts the UI holds (`incoming`) on top
/// of their threads, labelled for `clock`.
///
/// # Errors
///
/// Not linked, messages off for it, or the link failed.
pub async fn phone_threads(
    state: &AppState,
    device: DeviceId,
    before: Option<Cursor>,
    incoming: Vec<SmsMessage>,
    clock: LocalClock,
) -> Result<ThreadsView, CommandError> {
    let request = SmsRequest::Threads { before };
    match with_link(state, move |link| link.sms(&device, request.clone())).await? {
        SmsReply::ThreadsPage(threads) => Ok(threads_view(&threads, &incoming, clock)),
        _ => Err(unexpected_reply()),
    }
}

/// A page of `device`'s conversation `thread` after `before` (`None`:
/// the newest, with its provisional texts).
///
/// # Errors
///
/// As [`phone_threads`].
pub async fn phone_thread(
    state: &AppState,
    device: DeviceId,
    thread: i64,
    before: Option<Cursor>,
) -> Result<ThreadView, CommandError> {
    let request = SmsRequest::Thread { id: thread, before };
    match with_link(state, move |link| link.sms(&device, request.clone())).await? {
        SmsReply::ThreadPage(messages) => Ok(thread_view(&messages)),
        _ => Err(unexpected_reply()),
    }
}

/// Text `to` from `device`; sent at most once per `client_id`.
///
/// # Errors
///
/// As [`phone_threads`] (nothing is sent while the phone is away).
pub async fn send_sms(
    state: &AppState,
    device: DeviceId,
    to: String,
    text: String,
    client_id: u64,
) -> Result<SendOutcome, CommandError> {
    let request = SmsRequest::Send {
        to,
        text,
        client_id,
    };
    match with_link(state, move |link| link.sms(&device, request.clone())).await? {
        SmsReply::SendResult { outcome, .. } => Ok(outcome),
        _ => Err(unexpected_reply()),
    }
}

/// Carry out `action` on `device`'s call.
///
/// # Errors
///
/// Not linked, calls off for it, or the link failed.
pub async fn phone_call(
    state: &AppState,
    device: DeviceId,
    action: CallAction,
) -> Result<CallOutcome, CommandError> {
    with_link(state, move |link| link.call(&device, action.clone().into()))
        .await
        .map(CallOutcome::from)
}

/// A page of `device`'s call log after `before` (`None`: the newest),
/// by day at `clock`.
///
/// # Errors
///
/// As [`phone_call`].
pub async fn call_history(
    state: &AppState,
    device: DeviceId,
    before: Option<Cursor>,
    clock: LocalClock,
) -> Result<CallLogView, CommandError> {
    let command = CallCommand::History { before };
    match with_link(state, move |link| link.call(&device, command.clone())).await? {
        CallReply::HistoryPage(records) => Ok(call_log_view(&records, clock)),
        _ => Err(unexpected_reply()),
    }
}

/// What the call windows show of the engine's `view` at `now_ms`.
#[must_use]
pub fn call_window(view: &CallView, now_ms: i64) -> CallWindowView {
    call_view(view, now_ms)
}

/// The user's settings for showing a linked phone here.
///
/// # Errors
///
/// The supervisor is not ready or cannot be reached.
pub async fn phone_settings(state: &AppState) -> Result<PhoneSettings, CommandError> {
    with_link(state, SupervisorHandle::phone_settings).await
}

/// Save and apply the user's settings for showing a linked phone here.
///
/// # Errors
///
/// The supervisor is not ready or cannot be reached, or the settings
/// could not be saved.
pub async fn set_phone_settings(
    state: &AppState,
    settings: PhoneSettings,
) -> Result<(), CommandError> {
    with_link(state, move |link| link.set_phone_settings(settings)).await
}
