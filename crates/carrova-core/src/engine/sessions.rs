//! Session listing / maintenance (no live engine needed).

use std::io::Write;
use std::path::Path;

use carrova_platform::SemanticRoot;

use super::Engine;
use super::redact::REDACT_FILE;
use super::session::Session;
use crate::error::{CoreError, Result, SessionProblem};
use crate::rollback::{self, RollbackPreview, RollbackReport};
use crate::session::log::{Redaction, redact_log};
use crate::session::{Event, Role, SessionId, State};
use crate::store::{self, LOG_FILE, SessionRecord, Source, Store, read_file};
use crate::time;

/// All persisted sessions, newest first, after pruning expired undo
/// points ([`prune_expired`]; a failure there is a warning). Unreadable
/// records are reported as [`Event::Warning`]s on the engine sink and
/// skipped.
pub fn list(engine: &Engine) -> Result<Vec<SessionRecord>> {
    if let Err(e) = prune_expired(engine, time::now_unix()) {
        tracing::warn!(error = %e, "could not prune expired undo points");
        engine.sink().emit(Event::Warning {
            message: format!("could not remove expired undo points: {e}"),
        });
    }
    let platform = &**engine.platform();
    let (records, corrupt) = Store::open(platform)?.list_detailed(platform)?;
    for c in corrupt {
        tracing::warn!(session = %c.id, reason = %c.reason, "unreadable session record");
        engine.sink().emit(Event::Warning {
            message: format!("session `{}` is unreadable: {}", c.id, c.reason),
        });
    }
    Ok(records)
}

/// [`dismiss`] every session whose undo point expired at `now` (Unix
/// seconds; [`SessionRecord::expires_at`]) and that records no error;
/// returns their ids. Each session is re-checked under its lock, so one
/// whose rollback started meanwhile (which clears its expiry) is kept. A
/// session that cannot be dismissed is a warning (log + [`Event::Warning`])
/// and the others are still pruned.
///
/// # Errors
///
/// The store cannot be listed.
pub fn prune_expired(engine: &Engine, now: u64) -> Result<Vec<SessionId>> {
    let platform = &**engine.platform();
    let (records, _corrupt) = Store::open(platform)?.list_detailed(platform)?;
    let mut pruned = Vec::new();
    for r in records.into_iter().filter(|r| is_expired(r, now)) {
        match prune_one(engine, &r.id, now) {
            Ok(true) => pruned.push(r.id),
            Ok(false) => {}
            Err(e) => {
                tracing::warn!(session = %r.id, error = %e, "could not remove expired undo point");
                engine.sink().emit(Event::Warning {
                    message: format!("could not remove the expired undo point of `{}`: {e}", r.id),
                });
            }
        }
    }
    Ok(pruned)
}

/// Whether `r`'s undo point may be pruned at `now`: past its expiry and
/// with no error recorded (e.g. an incomplete rollback, whose backup may
/// hold the only copy of what it could not restore).
fn is_expired(r: &SessionRecord, now: u64) -> bool {
    r.error.is_none() && r.expires_at.is_some_and(|at| at <= now)
}

/// Dismiss `id` if, re-read under its lock, it is still expired at `now`;
/// whether it was.
fn prune_one(engine: &Engine, id: &SessionId, now: u64) -> Result<bool> {
    let (mut s, _lock) = Session::load_locked(engine.clone(), id)?;
    if !is_expired(&s.record, now) {
        return Ok(false);
    }
    tracing::info!(session = %id, "undo point expired");
    dismiss_session(&mut s)?;
    Ok(true)
}

/// One session.
pub fn show(engine: &Engine, id: &SessionId) -> Result<SessionRecord> {
    let platform = &**engine.platform();
    Store::open(platform)?.load(platform, id)
}

/// Roll back an import (latest `Done` import if `id` is None).
///
/// Accepted states: `Done` → `RolledBack`; `Applying` (a crashed apply;
/// no other process may be running it) → `RolledBack`; `Failed` with a
/// journal (e.g. an incomplete automatic rollback: retried, state stays
/// `Failed`). Steps that still fail are returned in
/// [`RollbackReport::failed`] and the state is left unchanged.
///
/// Before any step the session's undo point stops expiring
/// ([`SessionRecord::expires_at`] cleared and saved under the session
/// lock): from then on its backup may hold the only copy of what is not
/// yet restored, so only an explicit [`dismiss`] removes it.
pub fn rollback(engine: &Engine, id: Option<&SessionId>) -> Result<RollbackReport> {
    let id = rollback_id(engine, id)?;
    let mut s = {
        let (mut s, _lock) = Session::load_locked(engine.clone(), &id)?;
        check_rollbackable(&s)?;
        if s.record.expires_at.take().is_some() {
            s.save()?;
        }
        s
    };
    let from = s.record.state;
    let report = rollback::rollback(s.platform(), &s.dir, s.sink())?;
    s.wipe_staging();
    if !report.failed.is_empty() {
        s.record.error = Some(format!("rollback incomplete: {}", report.failure_summary()));
        s.save()?;
    } else if from == State::Failed {
        s.record.error = Some(format!(
            "{}; rolled back manually",
            s.record.error.as_deref().unwrap_or("failed")
        ));
        s.save()?;
    } else {
        s.transition(State::RolledBack)?;
    }
    Ok(report)
}

/// What [`rollback`] of the same session would do now — counts of paths
/// restored, removed and moved aside, plus the steps it would leave for
/// manual attention — without changing anything (no writes, no events,
/// no state change). Same sessions accepted as [`rollback`].
pub fn rollback_preview(engine: &Engine, id: Option<&SessionId>) -> Result<RollbackPreview> {
    let s = rollback_target(engine, id)?;
    rollback::preview(s.platform(), &s.dir)
}

/// The import [`rollback`] works on (`id`, else the latest `Done`
/// import), checked to be in a state that can be rolled back.
fn rollback_target(engine: &Engine, id: Option<&SessionId>) -> Result<Session> {
    let s = Session::load(engine.clone(), &rollback_id(engine, id)?)?;
    check_rollbackable(&s)?;
    Ok(s)
}

/// `id`, else the latest `Done` import.
fn rollback_id(engine: &Engine, id: Option<&SessionId>) -> Result<SessionId> {
    match id {
        Some(id) => Ok(id.clone()),
        None => list(engine)?
            .into_iter()
            .find(|r| r.role == Role::Import && r.state == State::Done)
            .map(|r| r.id)
            .ok_or(CoreError::NothingToRollBack),
    }
}

/// Refuse a session [`rollback`] cannot work on: an export, or an
/// import in a state with nothing to undo.
fn check_rollbackable(s: &Session) -> Result<()> {
    if s.record.role != Role::Import {
        return Err(s.problem(SessionProblem::ExportNotRollbackable));
    }
    let from = s.record.state;
    let rollbackable = match from {
        State::Done | State::Applying => true,
        State::Failed => s.has_journal()?,
        _ => false,
    };
    if !rollbackable {
        return Err(CoreError::InvalidTransition {
            from,
            to: State::RolledBack,
        });
    }
    Ok(())
}

/// The LAN session `--resume` continues: `given`, else the newest
/// resumable LAN session of `role` — an import in a non-terminal state
/// past scanning, or a send still `Transferring` (archive sessions never
/// count, so a newer archive import cannot hide an older LAN one).
/// Whether `given` fits is checked by the resume itself.
pub(super) fn lan_resume_target(
    engine: &Engine,
    given: Option<&SessionId>,
    role: Role,
) -> Result<SessionId> {
    if let Some(id) = given {
        return Ok(id.clone());
    }
    list(engine)?
        .into_iter()
        .find(|r| r.role == role && is_resumable_lan(r))
        .map(|r| r.id)
        .ok_or(CoreError::NothingToResume { role })
}

/// Whether `r` is a LAN session `--resume` can continue.
fn is_resumable_lan(r: &SessionRecord) -> bool {
    matches!(r.source, Some(Source::Lan { .. }))
        && match r.role {
            Role::Import => !r.state.is_terminal() && r.state != State::Scanned,
            Role::Export => r.state == State::Transferring,
        }
}

/// Delete a session's rollback point, staging and LAN resume secret.
/// Irreversible.
/// Refused while the session is `Applying`.
pub fn dismiss(engine: &Engine, id: &SessionId) -> Result<()> {
    let (mut s, _lock) = Session::load_locked(engine.clone(), id)?;
    dismiss_session(&mut s)
}

/// [`dismiss`] the loaded `s` (its lock held by the caller).
fn dismiss_session(s: &mut Session) -> Result<()> {
    if s.record.state == State::Applying {
        return Err(s.problem(SessionProblem::Applying));
    }
    rollback::dismiss(s.platform(), &s.dir)?;
    crate::stage::wipe(s.platform(), &s.staging(), &s.dir)?;
    store::wipe_resume_secret(s.platform(), &s.dir)?;
    if let Some(c) = s.record.checklist.as_mut() {
        c.rollback_available = false;
    }
    s.record.expires_at = None;
    s.save()
}

/// Write the session log to `out` (new file, mode 600) with strings
/// that mention a sensitive item's path replaced by `<redacted>` and the
/// home directory shown as `~`.
pub fn export_log(engine: &Engine, id: &SessionId, out: &Path) -> Result<()> {
    let platform = &**engine.platform();
    let store = Store::open(platform)?;
    store.load(platform, id)?;
    let dir = store.session_dir(id)?;
    let body = read_file(platform, &dir.join(LOG_FILE))?.unwrap_or_default();
    let redaction = Redaction {
        sensitive: store
            .read_json::<Vec<String>>(platform, id, REDACT_FILE)?
            .unwrap_or_default(),
        home: platform
            .resolve_root(SemanticRoot::Home)
            .ok()
            .map(|h| h.display().to_string()),
    };
    let text = redact_log(&body, &redaction);
    let mut f = platform.create_new_file(out, Some(0o600))?;
    f.write_all(text.as_bytes())?;
    f.sync_all()?;
    Ok(())
}
