//! Live sessions of this app run: each one's worker and the cancel token
//! of the object it owns (kept outside the worker so a cancel reaches a
//! running call).

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use carrova_core::{Cancel, Exporter, Importer, SendHandle, SessionId, TransferControl};

use crate::command_error::CommandError;
use crate::worker::Worker;

/// The sending side of a session, as it moves from selection to send.
#[derive(Debug)]
pub enum Sending {
    /// Scanned; the selection can still change (the engine refuses to
    /// export or send while its latest one is unusable).
    Selecting(Box<Exporter>),
    /// A send restored by resume, waiting to run.
    Resumed(Box<SendHandle>),
    /// The send ran, or failed to start.
    Ended,
}

/// A live session's worker.
#[derive(Debug, Clone)]
pub enum Live {
    /// Export or LAN send.
    Sending(Worker<Sending>),
    /// Archive or LAN import, with its transfer controls (kept outside
    /// the worker so pause/override reach a running apply).
    Importing(Worker<Importer>, TransferControl),
}

impl Live {
    fn is_busy(&self) -> bool {
        match self {
            Self::Sending(worker) => worker.is_busy(),
            Self::Importing(worker, _) => worker.is_busy(),
        }
    }
}

#[derive(Debug)]
struct Entry {
    /// `None` while the session object is being opened (resume).
    live: Option<Live>,
    cancel: Cancel,
}

/// Live sessions by id, plus the cancel token of a pairing in progress
/// (it has no session yet).
#[derive(Debug, Default)]
pub struct Registry {
    sessions: Mutex<HashMap<SessionId, Entry>>,
    connecting: Mutex<Option<Cancel>>,
}

impl Registry {
    /// Track `id` (replacing an earlier object of the same session, e.g.
    /// after a resume).
    pub fn insert(&self, id: SessionId, live: Live, cancel: Cancel) {
        let live = Some(live);
        self.lock().insert(id, Entry { live, cancel });
    }

    /// Track `id` while its object is being opened, so `cancel` reaches
    /// the opening (a resume reconnecting).
    pub fn opening(&self, id: SessionId, cancel: Cancel) {
        self.lock().insert(id, Entry { live: None, cancel });
    }

    /// Forget a session whose opening failed.
    pub fn abandon(&self, id: &SessionId) {
        self.lock().remove(id);
    }

    /// The sending worker of `id`.
    ///
    /// # Errors
    ///
    /// No such live sending session.
    pub fn sending(&self, id: &SessionId) -> Result<Worker<Sending>, CommandError> {
        match self.lock().get(id).and_then(|e| e.live.as_ref()) {
            Some(Live::Sending(worker)) => Ok(worker.clone()),
            _ => Err(CommandError::no_session("no such sending session is open")),
        }
    }

    /// The importing worker of `id`.
    ///
    /// # Errors
    ///
    /// No such live import session.
    pub fn importing(&self, id: &SessionId) -> Result<Worker<Importer>, CommandError> {
        match self.lock().get(id).and_then(|e| e.live.as_ref()) {
            Some(Live::Importing(worker, _)) => Ok(worker.clone()),
            _ => Err(CommandError::no_session("no such import session is open")),
        }
    }

    /// The transfer controls of import `id`.
    ///
    /// # Errors
    ///
    /// No such live import session.
    pub fn transfer(&self, id: &SessionId) -> Result<TransferControl, CommandError> {
        match self.lock().get(id).and_then(|e| e.live.as_ref()) {
            Some(Live::Importing(_, control)) => Ok(control.clone()),
            _ => Err(CommandError::no_session("no such import session is open")),
        }
    }

    /// Stop `id`'s running (or next) call. `false` if it is not live.
    pub fn cancel(&self, id: &SessionId) -> bool {
        self.lock().get(id).map(|e| e.cancel.cancel()).is_some()
    }

    /// Forget `id` before it is dismissed.
    ///
    /// # Errors
    ///
    /// A call of the session is still running (cancel it first).
    pub fn release(&self, id: &SessionId) -> Result<(), CommandError> {
        let mut sessions = self.lock();
        let running = sessions
            .get(id)
            .is_some_and(|e| e.live.as_ref().is_none_or(Live::is_busy));
        if running {
            return Err(CommandError::no_session("the session is still running")
                .with_hint("Cancel it first."));
        }
        sessions.remove(id);
        Ok(())
    }

    /// Whether a pairing, an opening or a session's call is running now.
    pub fn busy(&self) -> bool {
        self.connecting_lock().is_some()
            || self
                .lock()
                .values()
                .any(|e| e.live.as_ref().is_none_or(Live::is_busy))
    }

    /// Start a pairing: its cancel token, also kept for
    /// [`Self::cancel_connect`]. One pairing at a time.
    ///
    /// # Errors
    ///
    /// Another pairing is in progress.
    pub fn begin_connect(&self) -> Result<Cancel, CommandError> {
        let mut connecting = self.connecting_lock();
        if connecting.is_some() {
            return Err(CommandError::no_session("a pairing is already in progress"));
        }
        let cancel = Cancel::default();
        *connecting = Some(cancel.clone());
        Ok(cancel)
    }

    /// The pairing ended (paired or failed).
    pub fn end_connect(&self) {
        *self.connecting_lock() = None;
    }

    /// Stop the pairing in progress, if any.
    pub fn cancel_connect(&self) {
        if let Some(cancel) = &*self.connecting_lock() {
            cancel.cancel();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<SessionId, Entry>> {
        self.sessions.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn connecting_lock(&self) -> std::sync::MutexGuard<'_, Option<Cancel>> {
        self.connecting
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
