//! One live session: its persisted record, directory and log, and the only
//! place its state changes ([`Session::transition`]).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use carrova_platform::Platform;

use super::Engine;
use crate::error::{CoreError, Result, SessionProblem};
use crate::fsx;
use crate::journal::JOURNAL_FILE;
use crate::session::{Event, EventSink, LogSink, Role, SessionId, State};
use crate::store::{LOG_FILE, STAGING_DIR, SessionRecord, Source, Store};
use crate::{stage, time};

/// Persisted record + its directory, log and the transition helper.
///
/// `record.state` changes only through [`Self::transition`] (and
/// [`Self::fail`], which uses it); other record fields are set directly and
/// persisted by the next [`Self::save`] or transition.
#[derive(Debug)]
pub(super) struct Session {
    pub(super) engine: Engine,
    pub(super) store: Store,
    pub(super) dir: PathBuf,
    pub(super) record: SessionRecord,
    pub(super) log: Arc<LogSink>,
}

impl Session {
    /// A new persisted session of `role` in `state`.
    pub(super) fn create(
        engine: Engine,
        role: Role,
        state: State,
        source: Option<Source>,
    ) -> Result<Self> {
        let store = Store::open(&*engine.platform)?;
        let record = store.create(&*engine.platform, role, state, source)?;
        tracing::info!(session = %record.id, ?role, "session created");
        Self::with_record(engine, store, record)
    }

    /// The persisted session `id`.
    pub(super) fn load(engine: Engine, id: &SessionId) -> Result<Self> {
        let store = Store::open(&*engine.platform)?;
        let record = store.load(&*engine.platform, id)?;
        if record.id != *id {
            return Err(CoreError::RecordMismatch { id: id.clone() });
        }
        Self::with_record(engine, store, record)
    }

    fn with_record(engine: Engine, store: Store, record: SessionRecord) -> Result<Self> {
        let dir = store.session_dir(&record.id)?;
        let log = Arc::new(LogSink::open(
            &*engine.platform,
            &dir.join(LOG_FILE),
            Arc::clone(&engine.sink),
        )?);
        Ok(Self {
            engine,
            store,
            dir,
            record,
            log,
        })
    }

    pub(super) fn id(&self) -> &SessionId {
        &self.record.id
    }

    /// [`CoreError::Session`]: this session does not fit the request.
    pub(super) fn problem(&self, problem: SessionProblem) -> CoreError {
        CoreError::Session {
            id: self.id().clone(),
            problem,
        }
    }

    pub(super) fn platform(&self) -> &dyn Platform {
        &*self.engine.platform
    }

    /// The session's sink: logs, then forwards to the front-end.
    pub(super) fn sink(&self) -> &dyn EventSink {
        &*self.log
    }

    pub(super) fn emit(&self, event: Event) {
        self.log.emit(event);
    }

    /// Persist the record (stamping `updated_at`).
    pub(super) fn save(&mut self) -> Result<()> {
        self.record.updated_at = time::now_unix();
        self.store.save(&*self.engine.platform, &self.record)
    }

    /// Atomically write a JSON document into the session dir.
    pub(super) fn write_json<T: serde::Serialize>(&self, name: &str, value: &T) -> Result<()> {
        self.store
            .write_json(self.platform(), &self.record.id, name, value)
    }

    /// Read a JSON document from the session dir (`None` if absent).
    pub(super) fn read_json<T: serde::de::DeserializeOwned>(
        &self,
        name: &str,
    ) -> Result<Option<T>> {
        self.store.read_json(self.platform(), &self.record.id, name)
    }

    /// The only way state changes: check, persist, then emit.
    pub(super) fn transition(&mut self, to: State) -> Result<()> {
        let from = self.record.state;
        if !from.can_transition(to) {
            return Err(CoreError::InvalidTransition { from, to });
        }
        self.record.state = to;
        if let Err(e) = self.save() {
            self.record.state = from;
            return Err(e);
        }
        if from != to {
            tracing::info!(session = %self.record.id, ?from, ?to, "state changed");
            self.emit(Event::StateChanged {
                session: self.record.id.clone(),
                from,
                to,
            });
        }
        Ok(())
    }

    /// Fail with `err` unless the session already ended: record it and move
    /// to `Failed` (best effort; never masks `err`).
    pub(super) fn fail(&mut self, err: &CoreError) {
        if self.record.state == State::Failed || !self.record.state.can_transition(State::Failed) {
            return;
        }
        self.record.error = Some(err.to_string());
        if let Err(e) = self.transition(State::Failed) {
            tracing::warn!(error = %e, "could not persist failed state");
        }
    }

    /// Run `op`; if it fails, fail the session with its error.
    pub(super) fn failing<T>(&mut self, op: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        op(self).inspect_err(|e| self.fail(e))
    }

    /// The session's staging directory.
    pub(super) fn staging(&self) -> PathBuf {
        self.dir.join(STAGING_DIR)
    }

    /// Remove staging (best effort: a failure is a warning).
    pub(super) fn wipe_staging(&self) {
        if let Err(e) = stage::wipe(self.platform(), &self.staging(), &self.dir) {
            tracing::warn!(error = %e, "could not wipe staging");
            self.emit(Event::Warning {
                message: format!("could not remove staging data: {e}"),
            });
        }
    }

    /// Whether an apply journal (rollback point) exists.
    pub(super) fn has_journal(&self) -> Result<bool> {
        Ok(fsx::lstat(self.platform(), &self.dir.join(JOURNAL_FILE))?.is_some())
    }
}

/// `path` made absolute (unchanged if that fails), for the record.
pub(super) fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}
