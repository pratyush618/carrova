//! Linked devices: pairing a phone or computer, each device's features and
//! what flows over a link (clipboard, files, handoff, ring, asks). Every
//! call goes to the link supervisor in [`AppState::link`], the same one the
//! CLI reaches; its events reach the UI as [`BridgeEvent::Engine`] of
//! [`LINK_SESSION`], whether it runs here or in another process.

use std::path::PathBuf;
use std::sync::Arc;

use carrova_core::devices::DeviceRecord;
use carrova_core::ecosystem::{
    Answer, AskId, HandoffItem, PairInvite, PeerState, SharedSupervisor, SupervisorHandle,
    link_with_code, offer_pairing,
};
use carrova_core::lan::CODE_LIFETIME;
use carrova_core::time::now_unix;
use carrova_core::{Cancel, CodeOffer, CoreError, Engine, NullSink, SessionId};
use carrova_crypto::identity::DeviceId;
use carrova_link::{Feature, FeatureState};
use carrova_view::ecosystem::{
    ClipHistoryView, LinkedDeviceView, clip_history_view, linked_device_views, pair_invite_view,
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
fn link_engine(state: &AppState) -> Engine {
    Engine::new(Arc::clone(&state.platform), Vec::new(), Arc::new(NullSink))
}

/// Start the link supervisor, or attach to the one another process of this
/// user runs (again whenever that process exits), and serve it to the
/// commands. Blocking.
///
/// # Errors
///
/// The supervisor could not start, nor be reached.
pub fn start_link(state: &AppState) -> Result<(), CommandError> {
    serve_link(state, SharedSupervisor::attach(link_engine(state))?)
}

/// Serve `link` to the commands and forward its events to the UI. A run
/// keeps its first supervisor; a later one is dropped.
///
/// # Errors
///
/// The event thread could not start.
pub fn serve_link(state: &AppState, link: SharedSupervisor) -> Result<(), CommandError> {
    if state.link.set(link).is_err() {
        tracing::debug!("the link supervisor is already served");
        return Ok(());
    }
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

/// Show an invite for a phone to scan ([`BridgeEvent::PairInvite`]) and
/// wait until a phone links with it. `cancel_connect` stops it.
///
/// # Errors
///
/// Another pairing runs, it was cancelled, the invite expired, or the
/// pairing failed.
pub async fn pair_invite(state: &AppState) -> Result<(), CommandError> {
    let engine = link_engine(state);
    let out = Arc::clone(&state.out);
    pairing(state, move |cancel| {
        offer_pairing(&engine, cancel, |invite| {
            show_invite(&*out, &invite, cancel);
        })
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
    let engine = link_engine(state);
    pairing(state, move |cancel| {
        link_with_code(&engine, &ticket, cancel)
    })
    .await
}

/// Run the pairing `pair` as the one pairing in progress (cancellable by
/// `cancel_connect`, as a receive's).
async fn pairing(
    state: &AppState,
    pair: impl FnOnce(&Cancel) -> Result<DeviceRecord, CoreError> + Send + 'static,
) -> Result<(), CommandError> {
    let cancel = state.sessions.begin_connect()?;
    let paired = blocking(move || {
        pair(&cancel)
            .map(drop)
            .map_err(|e| CommandError::when_connecting(&e))
    })
    .await;
    state.sessions.end_connect();
    paired
}

/// Emit `invite`'s view; one that cannot be drawn ends the pairing.
fn show_invite(out: &dyn Emit, invite: &PairInvite, cancel: &Cancel) {
    let code = code_view(&invite.ticket, CodeOffer::First, CODE_LIFETIME);
    match pair_invite_view(invite, code) {
        Ok(invite) => out.emit(&BridgeEvent::PairInvite { invite }),
        Err(error) => {
            tracing::warn!(%error, "could not draw the pairing invite");
            cancel.cancel();
        }
    }
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
    // shortcut: the supervisor keeps no arrival times, so each clip shows
    // the time it was read; record them in core's history to fix.
    let now = now_unix();
    Ok(clip_history_view(clips.iter().map(|clip| (clip, now))))
}

/// Forget the clips received.
///
/// # Errors
///
/// The supervisor is not ready or cannot be reached.
pub async fn clear_clip_history(state: &AppState) -> Result<(), CommandError> {
    with_link(state, SupervisorHandle::clear_clip_history).await
}
