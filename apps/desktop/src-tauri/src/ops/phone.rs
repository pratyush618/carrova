//! A linked phone on this computer: its notifications, texts and calls,
//! and the user's settings for showing them. Each call goes to the link
//! supervisor (as in [`super::ecosystem`]); replies leave as the shared
//! phone view-models. The call-audio setup, the phone's permissions, its
//! notification app list and its messages sync window are this device's
//! own: they ask the platform (and the files the giver reads) directly.

use std::sync::Arc;

use carrova_core::ecosystem::{CallView, NotifyApps, PhoneSettings, SupervisorHandle, SyncWindow};
use carrova_crypto::identity::DeviceId;
use carrova_link::{
    ActionResult, CallCommand, CallReply, Feature, NotifyAction, SmsReply, SmsRequest,
};
use carrova_platform::PlatformError;
use carrova_platform::phone::{CallAudioStatus, Cursor, PhonePerm, SendOutcome, SmsMessage};
use carrova_view::ipc::{CallAction, CallOutcome, ErrorCode, NoteAction, NoteOutcome};
use carrova_view::phone::{
    CallLogView, CallSetupStep, CallWindowView, LocalClock, NotesView, NotifyAppRow,
    NotifyAppsView, PermStep, ThreadView, ThreadsView, call_audio_view, call_log_view, call_view,
    notes_view, notify_apps_view, perm_view, phone_apps_view, thread_view, threads_view,
    withheld_while_locked,
};

use super::ecosystem::with_link;
use crate::command_error::CommandError;
use crate::state::AppState;
use crate::worker::blocking;

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

/// `device`'s per-app notification list, read only (it is changed on
/// the phone).
///
/// # Errors
///
/// Not linked, notifications off for it, or the link failed.
pub async fn phone_apps(
    state: &AppState,
    device: DeviceId,
) -> Result<Vec<NotifyAppRow>, CommandError> {
    let apps = with_link(state, move |link| link.phone_apps(&device)).await?;
    Ok(phone_apps_view(&apps))
}

/// Have `device` remind its user to grant the access `feature` needs;
/// whether the reminder showed.
///
/// # Errors
///
/// `unsupported` for a feature other than notifications and messages;
/// not linked, the feature off for it, or the link failed.
pub async fn remind_phone(
    state: &AppState,
    device: DeviceId,
    feature: Feature,
) -> Result<bool, CommandError> {
    match feature {
        Feature::Notifications => {
            let done = with_link(state, move |link| {
                link.note_action(&device, NotifyAction::Remind)
            })
            .await?;
            Ok(done == ActionResult::Done)
        }
        Feature::Messages => {
            match with_link(state, move |link| link.sms(&device, SmsRequest::Remind)).await? {
                SmsReply::Reminded { posted } => Ok(posted),
                _ => Err(unexpected_reply()),
            }
        }
        _ => Err(CommandError::new(
            ErrorCode::Unsupported,
            "only notifications and messages need access on the phone",
        )),
    }
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

/// The call-audio setup screen this computer's state calls for. An OS
/// whose adapter can't tell reports it as unsupported, like one that says so.
///
/// # Errors
///
/// The OS could not be asked.
pub async fn call_audio_step(state: &AppState) -> Result<CallSetupStep, CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || {
        let status = match platform.call_audio_status() {
            Err(PlatformError::Unsupported(_)) => CallAudioStatus::Unsupported,
            status => status?,
        };
        Ok(call_audio_view(status))
    })
    .await
}

/// Install what hands-free call audio needs; the OS asks for the
/// password. Can take minutes, so it runs on the blocking pool.
///
/// # Errors
///
/// `cancelled` when the user dismissed the password prompt,
/// `unsupported` off Linux, the install failure otherwise.
pub async fn install_call_audio(state: &AppState) -> Result<(), CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || Ok(platform.install_call_audio()?)).await
}

/// Turn this computer's hands-free audio role on (`on`) or off (also
/// "Set up again"), once the user agreed.
///
/// # Errors
///
/// `unsupported` off Linux, the OS failure otherwise.
pub async fn set_call_audio_config(state: &AppState, on: bool) -> Result<(), CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || Ok(platform.set_call_audio_config(on)?)).await
}

/// What the phone shows for `perm`.
///
/// # Errors
///
/// `unsupported` off a phone, the OS failure otherwise.
pub async fn phone_perm_step(state: &AppState, perm: PhonePerm) -> Result<PermStep, CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || Ok(perm_view(perm, platform.phone_perm(perm)?))).await
}

/// Open the settings page where the user grants `perm`.
///
/// # Errors
///
/// As [`phone_perm_step`].
pub async fn open_perm_settings(state: &AppState, perm: PhonePerm) -> Result<(), CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || Ok(platform.open_perm_settings(perm)?)).await
}

/// On the phone: which apps' notifications reach linked devices, with
/// the apps of the notes showing now (none without notification access).
///
/// # Errors
///
/// The saved list cannot be read.
pub async fn notify_apps(state: &AppState) -> Result<NotifyAppsView, CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || {
        let apps = NotifyApps::open(&*platform)?;
        let showing = platform.active_notes().unwrap_or_default();
        Ok(notify_apps_view(&apps, &showing))
    })
    .await
}

/// On the phone: turn app `app_id`'s notifications on or off.
///
/// # Errors
///
/// The list cannot be read or saved.
pub async fn set_notify_app(
    state: &AppState,
    app_id: String,
    on: bool,
) -> Result<(), CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || Ok(NotifyApps::open(&*platform)?.set(&*platform, &app_id, on)?)).await
}

/// On the phone: hide one-time codes, send message text.
///
/// # Errors
///
/// As [`set_notify_app`].
pub async fn set_notify_filters(
    state: &AppState,
    hide_codes: bool,
    show_text: bool,
) -> Result<(), CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || {
        Ok(NotifyApps::open(&*platform)?.set_filters(&*platform, hide_codes, show_text)?)
    })
    .await
}

/// On the phone: how far back texts reach linked devices.
///
/// # Errors
///
/// The saved choice cannot be read.
pub async fn messages_sync(state: &AppState) -> Result<SyncWindow, CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || Ok(SyncWindow::open(&*platform)?)).await
}

/// On the phone: save how far back texts reach linked devices.
///
/// # Errors
///
/// The choice cannot be saved.
pub async fn set_messages_sync(state: &AppState, keep: SyncWindow) -> Result<(), CommandError> {
    let platform = Arc::clone(&state.platform);
    blocking(move || Ok(keep.save(&*platform)?)).await
}
