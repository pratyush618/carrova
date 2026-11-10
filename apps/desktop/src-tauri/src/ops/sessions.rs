//! Any session: cancel, resume, history, rollback, dismiss.

use std::sync::Arc;

use carrova_core::engine::sessions;
use carrova_core::{
    Cancel, Engine, Exporter, Importer, Role, RollbackPreview, RollbackReport, SessionId,
    SessionRecord, Source,
};
use carrova_view::ipc::{ErrorCode, SessionOpened};
use carrova_view::passphrase::{PassphraseUse, archive_passphrase};

use crate::command_error::CommandError;
use crate::events::SessionSink;
use crate::registry::{Live, Sending};
use crate::state::AppState;
use crate::worker::{Worker, blocking};

/// An engine for history and maintenance calls, which read and repair
/// session state but never use recipes (so history works even when no
/// recipes are available).
fn maintenance(state: &AppState, sink: Arc<SessionSink>) -> Engine {
    Engine::new(Arc::clone(&state.platform), Vec::new(), sink)
}

/// Stop `id`'s running task between steps (never mid-apply). A pending
/// "show a new code?" question is answered "no".
///
/// # Errors
///
/// The session is not live in this app.
pub fn cancel(state: &AppState, id: &SessionId) -> Result<(), CommandError> {
    if !state.sessions.cancel(id) {
        return Err(CommandError::no_session("no such session is running"));
    }
    // No question open is the normal case.
    let _ = state.answers.answer(id, false);
    Ok(())
}

/// Stop the pairing in progress (`connect`), if any.
pub fn cancel_connect(state: &AppState) {
    state.sessions.cancel_connect();
}

/// Resume the interrupted session `id`: a LAN send waits for its paired
/// receiver (`run_send` runs it), a LAN receive reconnects, an archive
/// import reopens its archive (needs `passphrase`). The UI then plans
/// (receive) or runs (send) as for a new session.
///
/// # Errors
///
/// Nothing to resume, archive passphrase missing or wrong, sender
/// unreachable, cancel.
pub async fn resume(
    state: &AppState,
    id: &SessionId,
    passphrase: Option<String>,
) -> Result<SessionOpened, CommandError> {
    let record = {
        let engine = maintenance(state, state.unbound_sink());
        let id = id.clone();
        blocking(move || Ok(sessions::show(&engine, &id)?)).await?
    };
    let connecting = Cancel::default();
    state.sessions.opening(id.clone(), connecting.clone());
    match open_resumed(state, &record, passphrase, connecting).await {
        Ok((live, cancel)) => {
            state.sessions.insert(id.clone(), live, cancel);
            Ok(SessionOpened {
                session: id.clone(),
            })
        }
        Err(error) => {
            state.sessions.abandon(id);
            Err(error)
        }
    }
}

/// Reopen the session object of `record` on a new worker; returns it
/// with its cancel token. `connecting` stops a LAN receive's reconnect.
async fn open_resumed(
    state: &AppState,
    record: &SessionRecord,
    passphrase: Option<String>,
    connecting: Cancel,
) -> Result<(Live, Cancel), CommandError> {
    let shared = state.clone();
    let id = record.id.clone();
    let engine = move || shared.engine(shared.sink_for(&id)).map(|e| (e, id));
    match (record.role, &record.source) {
        (Role::Export, _) => {
            let (worker, cancel) = Worker::open("send", move || {
                let (engine, id) = engine()?;
                let handle = Exporter::resume_send(engine, Some(&id))?;
                let cancel = handle.canceller();
                Ok::<_, CommandError>((Sending::Resumed(Box::new(handle)), cancel))
            })
            .await?;
            Ok((Live::Sending(worker), cancel))
        }
        (Role::Import, Some(Source::Lan { .. })) => {
            let cancel = connecting.clone();
            let (worker, control) = Worker::open("receive", move || {
                let (engine, id) = engine()?;
                let importer = Importer::resume_lan_with(engine, Some(&id), connecting)?;
                let control = importer.transfer_control();
                Ok::<_, CommandError>((importer, control))
            })
            .await?;
            Ok((Live::Importing(worker, control), cancel))
        }
        (Role::Import, _) => {
            let typed = passphrase.ok_or_else(|| {
                CommandError::new(
                    ErrorCode::WrongPassphrase,
                    "enter the archive passphrase to continue",
                )
            })?;
            let passphrase = archive_passphrase(typed, PassphraseUse::ExistingArchive)?;
            let (worker, (cancel, control)) = Worker::open("import", move || {
                let (engine, id) = engine()?;
                let importer = Importer::resume_archive(engine, &id, &passphrase)?;
                let handles = super::receive::handles(&importer);
                Ok::<_, CommandError>((importer, handles))
            })
            .await?;
            Ok((Live::Importing(worker, control), cancel))
        }
    }
}

/// Every session, newest first.
///
/// # Errors
///
/// The session store cannot be read.
pub async fn list(state: &AppState) -> Result<Vec<SessionRecord>, CommandError> {
    let engine = maintenance(state, state.unbound_sink());
    blocking(move || Ok(sessions::list(&engine)?)).await
}

/// Undo the applied import `id` (the newest one when `None`).
///
/// # Errors
///
/// Nothing to roll back, or the rollback failed.
pub async fn rollback(
    state: &AppState,
    id: Option<SessionId>,
) -> Result<RollbackReport, CommandError> {
    let sink = match &id {
        Some(id) => state.sink_for(id),
        None => state.unbound_sink(),
    };
    let engine = maintenance(state, sink);
    blocking(move || Ok(sessions::rollback(&engine, id.as_ref())?)).await
}

/// What undoing the applied import `id` (the newest one when `None`)
/// would do, without changing anything.
///
/// # Errors
///
/// Nothing to roll back, or the rollback point cannot be read.
pub async fn rollback_preview(
    state: &AppState,
    id: Option<SessionId>,
) -> Result<RollbackPreview, CommandError> {
    let engine = maintenance(state, state.unbound_sink());
    blocking(move || Ok(sessions::rollback_preview(&engine, id.as_ref())?)).await
}

/// Forget `id` and its staged data (and rollback point).
///
/// # Errors
///
/// The session is still running here, or it cannot be removed.
pub async fn dismiss(state: &AppState, id: &SessionId) -> Result<(), CommandError> {
    state.sessions.release(id)?;
    let engine = maintenance(state, state.sink_for(id));
    let id = id.clone();
    blocking(move || Ok(sessions::dismiss(&engine, &id)?)).await
}
