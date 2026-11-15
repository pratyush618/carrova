//! Linked devices: pairing a phone or computer, each device's features and
//! what flows over a link (clipboard, files, handoff, ring, asks). Every
//! call goes to the link supervisor in [`AppState::link`], the same one the
//! CLI reaches; its events reach the UI as [`BridgeEvent::Engine`] of
//! [`LINK_SESSION`], whether it runs here or in another process.

use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex, OnceLock, PoisonError};

use carrova_core::devices::DeviceRecord;
use carrova_core::ecosystem::{
    Answer, AskId, ClipSettings, HandoffItem, PAIR_CONFIRM_WINDOW, PairInvite, PeerState,
    SharedSupervisor, SupervisorHandle, link_with_code, offer_pairing,
};
use carrova_core::lan::CODE_LIFETIME;
use carrova_core::{Cancel, CodeOffer, CoreError, Engine, NullSink, SessionId};
use carrova_crypto::identity::DeviceId;
use carrova_link::{Feature, FeatureState};
use carrova_platform::Platform;
use carrova_view::ecosystem::{
    ClipHistoryView, LinkedDeviceView, QrError, clip_history_view, linked_device_views,
    pair_invite_view, pair_peer_view,
};
use carrova_view::ipc::{BridgeEvent, ErrorCode, LINK_SESSION};
use carrova_view::lan::code_view;

use crate::command_error::CommandError;
use crate::events::Emit;
use crate::ops::receive::ticket;
use crate::state::AppState;
use crate::worker::blocking;

/// An engine for linked-device calls: they use no recipes, and the
/// supervisor's events arrive through [`serve_link`]'s subscription, so the
/// engine's own sink would only repeat them.
fn link_engine(platform: &Arc<dyn Platform>) -> Engine {
    Engine::new(Arc::clone(platform), Vec::new(), Arc::new(NullSink))
}

/// This process's one link supervisor slot, shared by every start point:
/// the app's `setup` and, on a phone, the link service, which may start
/// first (no window yet) or alone (after boot).
static PROCESS_LINK: LazyLock<Arc<OnceLock<SharedSupervisor>>> = LazyLock::new(Arc::default);

/// The process's link supervisor slot ([`AppState::link`] of the app).
#[must_use]
pub fn process_link() -> Arc<OnceLock<SharedSupervisor>> {
    Arc::clone(&PROCESS_LINK)
}

/// Fill `link` with `start`'s supervisor unless it holds one already, and
/// return it. Starts are serialised, so racing starters start one; a failed
/// start leaves `link` empty for the next try. Blocking.
///
/// # Errors
///
/// `start`'s error.
pub fn start_once(
    link: &OnceLock<SharedSupervisor>,
    start: impl FnOnce() -> Result<SharedSupervisor, CoreError>,
) -> Result<&SharedSupervisor, CoreError> {
    static STARTING: Mutex<()> = Mutex::new(());
    let _one = STARTING.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(running) = link.get() {
        return Ok(running);
    }
    let started = start()?;
    Ok(link.get_or_init(|| started))
}

/// Start this process's supervisor, or attach to the one another process
/// of this user runs, with no app around it (a phone's link service).
/// Blocking.
///
/// # Errors
///
/// The supervisor could not start, nor be reached.
pub fn start_process_link(platform: &Arc<dyn Platform>) -> Result<(), CoreError> {
    start_once(&PROCESS_LINK, || {
        SharedSupervisor::attach(link_engine(platform))
    })
    .map(drop)
}

/// Start the link supervisor (attaching again whenever its host process
/// exits), unless it already runs, and serve it to the commands. Blocking.
///
/// # Errors
///
/// The supervisor could not start, nor be reached.
pub fn start_link(state: &AppState) -> Result<(), CommandError> {
    serve_link(state, || {
        SharedSupervisor::attach(link_engine(&state.platform))
    })
}

/// Serve [`AppState::link`] to the commands, started with `start` unless
/// it runs already, and forward its events to the UI. Once per state.
///
/// # Errors
///
/// `start` failed, or the event thread could not start.
pub fn serve_link(
    state: &AppState,
    start: impl FnOnce() -> Result<SharedSupervisor, CoreError>,
) -> Result<(), CommandError> {
    start_once(&state.link, start)?;
    let sink = state.sink_for(&SessionId(LINK_SESSION.to_owned()));
    let link = Arc::clone(&state.link);
    std::thread::Builder::new()
        .name("carrova-link-events".to_owned())
        .spawn(move || {
            if let Some(link) = link.get() {
                link.forward(&*sink);
            }
        })?;
    Ok(())
}

/// The devices linked now, for the tray. Blocking; empty (logged) when the
/// supervisor cannot tell.
pub fn linked_now(state: &AppState) -> Vec<LinkedDeviceView> {
    let Some(link) = state.link.get() else {
        return Vec::new();
    };
    match link.call(SupervisorHandle::status) {
        Ok(peers) => linked_device_views(&peers)
            .into_iter()
            .filter(|device| device.state == PeerState::Linked)
            .collect(),
        Err(error) => {
            tracing::warn!(%error, "could not list the linked devices");
            Vec::new()
        }
    }
}

/// Run `call` on the served supervisor, off the async runtime.
async fn with_link<R: Send + 'static>(
    state: &AppState,
    call: impl Fn(&SupervisorHandle) -> Result<R, CoreError> + Send + 'static,
) -> Result<R, CommandError> {
    let link = Arc::clone(&state.link);
    blocking(move || {
        let link = link.get().ok_or_else(|| {
            CommandError::new(ErrorCode::Network, "linked devices are not ready yet")
                .with_hint("Try again in a moment.")
        })?;
        Ok(link.call(call)?)
    })
    .await
}

/// Every known device and its link.
///
/// # Errors
///
/// The supervisor is not ready or cannot be reached.
pub async fn linked_devices(state: &AppState) -> Result<Vec<LinkedDeviceView>, CommandError> {
    with_link(state, SupervisorHandle::status)
        .await
        .map(|peers| linked_device_views(&peers))
}

/// Show an invite for a phone to scan ([`BridgeEvent::PairInvite`]),
/// show the device that answers it ([`BridgeEvent::PairPeerFound`]) and
/// link it once `answer_pair_peer` confirms. `cancel_connect` stops it.
///
/// # Errors
///
/// Another pairing runs, the invite could not be drawn, it was
/// cancelled, the invite expired, the user did not confirm the device,
/// or the pairing failed.
pub async fn pair_invite(state: &AppState) -> Result<(), CommandError> {
    let engine = link_engine(&state.platform);
    let out = Arc::clone(&state.out);
    let peer_answer = Arc::clone(&state.peer_answer);
    pairing(state, move |cancel| {
        let mut undrawn = None;
        let on_code = |invite: PairInvite| {
            if let Err(error) = show_invite(&*out, &invite) {
                undrawn = Some(error);
                cancel.cancel();
            }
        };
        let on_peer =
            |found: &_| peer_answer.ask(&*out, pair_peer_view(found), cancel, PAIR_CONFIRM_WINDOW);
        let paired = offer_pairing(&engine, cancel, on_code, on_peer);
        match undrawn {
            Some(error) => Err(error.into()),
            None => paired.map_err(|e| CommandError::when_connecting(&e)),
        }
    })
    .await
}

/// Link with the computer showing `code` ("Link a computer").
/// `cancel_connect` stops it.
///
/// # Errors
///
/// Not a pairing code, another pairing runs, it was cancelled, or the
/// pairing failed.
pub async fn link_computer(state: &AppState, code: String) -> Result<(), CommandError> {
    let ticket = ticket(code)?;
    let engine = link_engine(&state.platform);
    pairing(state, move |cancel| {
        link_with_code(&engine, &ticket, cancel).map_err(|e| CommandError::when_connecting(&e))
    })
    .await
}

/// Answer the open "is this your device?" question of `pair_invite`.
///
/// # Errors
///
/// No question is open.
pub fn answer_pair_peer(state: &AppState, accept: bool) -> Result<(), CommandError> {
    state
        .peer_answer
        .answer(accept)
        .map_err(|_| CommandError::no_session("no device is waiting to be confirmed"))
}

/// Run the pairing `pair` as the one pairing in progress (cancellable by
/// `cancel_connect`, as a receive's).
async fn pairing(
    state: &AppState,
    pair: impl FnOnce(&Cancel) -> Result<DeviceRecord, CommandError> + Send + 'static,
) -> Result<(), CommandError> {
    let cancel = state.sessions.begin_connect()?;
    let paired = blocking(move || pair(&cancel).map(drop)).await;
    state.sessions.end_connect();
    paired
}

/// Emit `invite`'s view.
fn show_invite(out: &dyn Emit, invite: &PairInvite) -> Result<(), QrError> {
    let code = code_view(&invite.ticket, CodeOffer::First, CODE_LIFETIME);
    let invite = pair_invite_view(invite, code)?;
    out.emit(&BridgeEvent::PairInvite { invite });
    Ok(())
}

/// Set `feature` to `feature_state` for `device`.
///
/// # Errors
///
/// Unknown device, a state the feature does not take, or the policy
/// could not be saved.
pub async fn set_feature(
    state: &AppState,
    device: DeviceId,
    feature: Feature,
    feature_state: FeatureState,
) -> Result<(), CommandError> {
    with_link(state, move |link| {
        link.set_feature(&device, feature, feature_state)
    })
    .await
}

/// Answer an ask (`AskReceived`): accept or decline.
///
/// # Errors
///
/// It was already answered or expired.
pub async fn answer_ask(state: &AppState, ask: AskId, accept: bool) -> Result<(), CommandError> {
    let answer = if accept {
        Answer::Accept
    } else {
        Answer::Decline
    };
    with_link(state, move |link| link.answer(ask, answer)).await
}

/// Send this clipboard to `device`.
///
/// # Errors
///
/// Not linked, clipboard off for it, or the link failed.
pub async fn send_clipboard(state: &AppState, device: DeviceId) -> Result<(), CommandError> {
    with_link(state, move |link| link.send_clipboard(&device)).await
}

/// Offer `paths` to `device` one by one, each sent once accepted there.
/// Paths are resolved first, as the CLI does.
///
/// # Errors
///
/// A path is not a readable file, not linked, files off for it, declined,
/// expired, or the link failed. Files before the failing one were sent.
pub async fn send_files(
    state: &AppState,
    device: DeviceId,
    paths: Vec<PathBuf>,
) -> Result<(), CommandError> {
    with_link(state, move |link| {
        for path in &paths {
            link.send_file(&device, &std::fs::canonicalize(path)?)?;
        }
        Ok(())
    })
    .await
}

/// Open `url` (`http(s)`) on `device`, once its user accepts.
///
/// # Errors
///
/// Not linked, handoff off for it, not a web link, declined, or the link
/// failed.
pub async fn handoff_url(
    state: &AppState,
    device: DeviceId,
    url: String,
) -> Result<(), CommandError> {
    with_link(state, move |link| {
        link.handoff(&device, HandoffItem::Url(url.clone()))
    })
    .await
}

/// Make `device` ring.
///
/// # Errors
///
/// Not linked, ring off for it, or the link failed.
pub async fn ring(state: &AppState, device: DeviceId) -> Result<(), CommandError> {
    with_link(state, move |link| link.ring(&device)).await
}

/// The clips received, newest first.
///
/// # Errors
///
/// The supervisor is not ready or cannot be reached.
pub async fn clip_history(state: &AppState) -> Result<ClipHistoryView, CommandError> {
    let clips = with_link(state, SupervisorHandle::clip_history).await?;
    Ok(clip_history_view(
        clips.iter().map(|clip| (&clip.item, clip.at)),
    ))
}

/// Forget the clips received.
///
/// # Errors
///
/// The supervisor is not ready or cannot be reached.
pub async fn clear_clip_history(state: &AppState) -> Result<(), CommandError> {
    with_link(state, SupervisorHandle::clear_clip_history).await
}

/// The user's clipboard settings.
///
/// # Errors
///
/// The supervisor is not ready or cannot be reached.
pub async fn clip_settings(state: &AppState) -> Result<ClipSettings, CommandError> {
    with_link(state, SupervisorHandle::clip_settings).await
}

/// Save and apply the user's clipboard settings.
///
/// # Errors
///
/// The supervisor is not ready or cannot be reached, or the settings
/// could not be saved.
pub async fn set_clip_settings(
    state: &AppState,
    settings: ClipSettings,
) -> Result<(), CommandError> {
    with_link(state, move |link| link.set_clip_settings(settings)).await
}

/// Send file `id` again: one whose device left mid-send.
///
/// # Errors
///
/// No such file waits to be sent again, or as [`send_files`].
pub async fn retry_file(state: &AppState, id: u64) -> Result<(), CommandError> {
    with_link(state, move |link| link.retry_file(id)).await
}
