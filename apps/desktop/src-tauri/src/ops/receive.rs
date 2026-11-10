//! Receiving side: open an archive or pair with a sender, then plan,
//! approve and apply.
//!
//! Review rules stay in the engine: `approve` refuses a plan with
//! blockers ([`carrova_core::CoreError::PlanBlocked`], code `blocked`),
//! `apply` refuses an unapproved plan, and a new plan always needs a new
//! approval.

use std::path::PathBuf;

use carrova_core::lan::browse_nearby;
use carrova_core::{
    Cancel, Checklist, Importer, PlanEdit, PlanOptions, SessionId, Ticket, TransferControl,
};
use carrova_view::ipc::{ErrorCode, PlanReply, SessionOpened};
use carrova_view::lan::{NearbyPeer, nearby_peer};
use carrova_view::passphrase::{PassphraseUse, archive_passphrase};
use carrova_view::review;
use zeroize::Zeroizing;

use super::task_ended;
use crate::command_error::CommandError;
use crate::registry::Live;
use crate::state::AppState;
use crate::worker::{Worker, blocking};

/// Track the import session `importer` opened on `worker`.
fn opened(
    state: &AppState,
    worker: Worker<Importer>,
    id: SessionId,
    (cancel, control): (Cancel, TransferControl),
) -> SessionOpened {
    state
        .sessions
        .insert(id.clone(), Live::Importing(worker, control), cancel);
    SessionOpened { session: id }
}

/// The handles of `importer` the registry keeps outside its worker.
pub(super) fn handles(importer: &Importer) -> (Cancel, TransferControl) {
    (importer.canceller(), importer.transfer_control())
}

/// Open the archive at `path` as a new import session.
///
/// # Errors
///
/// Wrong passphrase, not an archive, or recipes unavailable.
pub async fn open_archive(
    state: &AppState,
    path: PathBuf,
    passphrase: String,
) -> Result<SessionOpened, CommandError> {
    let passphrase = archive_passphrase(passphrase, PassphraseUse::ExistingArchive)?;
    let shared = state.clone();
    let (worker, (id, handles)) = Worker::open("import", move || {
        let sink = shared.unbound_sink();
        let importer = Importer::open(shared.engine(sink.clone())?, &path, &passphrase)?;
        sink.bind(importer.id());
        let opened = (importer.id().clone(), handles(&importer));
        Ok::<_, CommandError>((importer, opened))
    })
    .await?;
    Ok(opened(state, worker, id, handles))
}

/// Parse the code (or `code@ip:port` ticket) the user typed. The input is
/// wiped afterwards and never echoed in the error.
fn ticket(code: String) -> Result<Ticket, CommandError> {
    let code = Zeroizing::new(code);
    code.trim().parse().map_err(|_| {
        CommandError::new(ErrorCode::BadCode, "that is not a pairing code").with_hint(
            "Type it as shown on the sending machine, e.g. 7-word-word or \
             7-word-word@192.168.1.20:47822.",
        )
    })
}

/// Pair with a sender by its code: a new LAN import session once the
/// sender's manifest arrived. `cancel_connect` stops it while pairing.
///
/// # Errors
///
/// Malformed or rejected code, sender not found, another pairing in
/// progress, cancel.
pub async fn connect(state: &AppState, code: String) -> Result<SessionOpened, CommandError> {
    let ticket = ticket(code)?;
    let cancel = state.sessions.begin_connect()?;
    let shared = state.clone();
    let pairing = cancel.clone();
    let result = Worker::open("receive", move || {
        let sink = shared.unbound_sink();
        let importer = Importer::connect_with(shared.engine(sink.clone())?, &ticket, pairing)
            .map_err(|e| CommandError::when_connecting(&e))?;
        sink.bind(importer.id());
        let opened = (importer.id().clone(), importer.transfer_control());
        Ok::<_, CommandError>((importer, opened))
    })
    .await;
    state.sessions.end_connect();
    let (worker, (id, control)) = result?;
    Ok(opened(state, worker, id, (cancel, control)))
}

/// The plan for the user's `options` and everything the review shows
/// ([`carrova_view::review::prepare`]: a resumed session's plan approved
/// with the same options stays approved). Writes nothing outside Carrova's
/// session directory.
///
/// # Errors
///
/// No such import session, or not in a state that can plan.
pub async fn plan(
    state: &AppState,
    id: &SessionId,
    options: PlanOptions,
) -> Result<PlanReply, CommandError> {
    let platform = state.platform.clone();
    state
        .sessions
        .importing(id)?
        .call(move |importer| {
            let prepared = review::prepare(importer, platform.as_ref(), &options)?;
            Ok(PlanReply::new(prepared, importer.manifest().cloned()))
        })
        .await?
}

/// The current plan with the user's `edits` applied to its options
/// ([`carrova_core::Importer::edited_options`]), reviewed like [`plan`].
///
/// # Errors
///
/// Nothing planned yet, an edit names an unknown item or plugin, or as
/// [`plan`].
pub async fn edit_plan(
    state: &AppState,
    id: &SessionId,
    edits: Vec<PlanEdit>,
) -> Result<PlanReply, CommandError> {
    let platform = state.platform.clone();
    state
        .sessions
        .importing(id)?
        .call(move |importer| {
            let options = importer.edited_options(edits)?;
            let prepared = review::prepare(importer, platform.as_ref(), &options)?;
            Ok(PlanReply::new(prepared, importer.manifest().cloned()))
        })
        .await?
}

/// The options the session last planned with (`None` before its first
/// plan). A resumed session continues from them, so the user's earlier
/// choices (confirmed items, rules) and its approval carry over; new moves
/// start from the suggestions instead.
///
/// # Errors
///
/// No such import session.
pub async fn plan_options(
    state: &AppState,
    id: &SessionId,
) -> Result<Option<PlanOptions>, CommandError> {
    state
        .sessions
        .importing(id)?
        .call(|importer| importer.plan_options().cloned())
        .await
}

/// Approve the current plan (a LAN sender is told).
///
/// # Errors
///
/// `blocked` while the plan has blockers; no plan; peer unreachable.
pub async fn approve(state: &AppState, id: &SessionId) -> Result<(), CommandError> {
    state
        .sessions
        .importing(id)?
        .call(|importer| Ok(importer.approve()?))
        .await?
}

/// Apply the approved plan: staged, journalled, reversible.
///
/// # Errors
///
/// Not approved, transfer failure, cancel before writing, or an apply
/// failure (rolled back).
pub async fn apply(state: &AppState, id: &SessionId) -> Result<Checklist, CommandError> {
    let worker = state.sessions.importing(id)?;
    let result = worker
        .call(|importer| Ok(importer.apply()?))
        .await
        .and_then(|r| r);
    task_ended(state, id, result)
}

/// Pause (`true`) or continue (`false`) the running LAN transfer of
/// import `id` between batches.
///
/// # Errors
///
/// No such live import session.
pub fn set_transfer_paused(
    state: &AppState,
    id: &SessionId,
    paused: bool,
) -> Result<(), CommandError> {
    let control = state.sessions.transfer(id)?;
    if paused {
        control.pause();
    } else {
        control.resume();
    }
    Ok(())
}

/// Reconnect the LAN transfer of import `id` now instead of after the
/// back-off (the user's "Try now" while the sender is unreachable).
///
/// # Errors
///
/// No such live import session.
pub fn retry_connection(state: &AppState, id: &SessionId) -> Result<(), CommandError> {
    state.sessions.transfer(id)?.retry_now();
    Ok(())
}

/// Senders nearby offering a pairing code (device names only).
///
/// # Errors
///
/// Discovery could not start.
pub async fn nearby(state: &AppState) -> Result<Vec<NearbyPeer>, CommandError> {
    let platform = state.platform.clone();
    blocking(move || {
        Ok(browse_nearby(platform.as_ref())?
            .iter()
            .map(nearby_peer)
            .collect())
    })
    .await
}

/// Continue the LAN transfer of import `id` despite low free space (the
/// user's override, given after the warning). The disk filling up for
/// real still stops it.
///
/// # Errors
///
/// No such live import session.
pub fn ignore_free_space(state: &AppState, id: &SessionId) -> Result<(), CommandError> {
    state.sessions.transfer(id)?.ignore_free_space();
    Ok(())
}

/// End the review without applying (a LAN sender is told nothing will
/// be transferred).
///
/// # Errors
///
/// No such import session, or the sender could not be told.
pub async fn end_review(state: &AppState, id: &SessionId) -> Result<(), CommandError> {
    state
        .sessions
        .importing(id)?
        .call(|importer| Ok(importer.end_dry_run()?))
        .await?
}
