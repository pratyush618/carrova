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

/// All persisted sessions, newest first. Unreadable records are
/// reported as [`Event::Warning`]s on the engine sink and skipped.
pub fn list(engine: &Engine) -> Result<Vec<SessionRecord>> {
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
pub fn rollback(engine: &Engine, id: Option<&SessionId>) -> Result<RollbackReport> {
    let mut s = rollback_target(engine, id)?;
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
    let id = match id {
        Some(id) => id.clone(),
        None => list(engine)?
            .into_iter()
            .find(|r| r.role == Role::Import && r.state == State::Done)
            .map(|r| r.id)
            .ok_or(CoreError::NothingToRollBack)?,
    };
    let s = Session::load(engine.clone(), &id)?;
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
    Ok(s)
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
    let mut s = Session::load(engine.clone(), id)?;
    if s.record.state == State::Applying {
        return Err(s.problem(SessionProblem::Applying));
    }
    rollback::dismiss(s.platform(), &s.dir)?;
    crate::stage::wipe(s.platform(), &s.staging(), &s.dir)?;
    store::wipe_resume_secret(s.platform(), &s.dir)?;
    if let Some(c) = s.record.checklist.as_mut() {
        c.rollback_available = false;
    }
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
