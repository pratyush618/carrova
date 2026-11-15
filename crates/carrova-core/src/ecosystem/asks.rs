//! Per-offer approvals (spec "Policy"): one ask per offer, answered by the
//! user or expired after [`SupervisorConfig::ask_expiry`], never
//! remembered.
//!
//! [`SupervisorConfig::ask_expiry`]: super::SupervisorConfig::ask_expiry

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Mutex, PoisonError};
use std::time::{Duration, Instant};

use carrova_crypto::identity::DeviceId;
use serde::{Deserialize, Serialize};

use crate::cancel::Cancel;
use crate::error::{CoreError, Result};
use crate::session::{Event, EventSink};

/// How often a waiting ask checks whether its link closed.
const SLICE: Duration = Duration::from_millis(100);

/// One pending ask ([`Event::AskReceived`]), answered with
/// [`Supervisor::answer`](super::Supervisor::answer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(transparent)]
pub struct AskId(u64);

/// The user's answer to an ask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Answer {
    /// Take the file or open the link.
    Accept,
    /// Refuse it.
    Decline,
}

/// What a device offers, as shown in the ask. Names are already safe
/// ([`carrova_link::safe_file_name`]); a URL shows its host only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AskKind {
    /// A file to save in Downloads.
    File {
        /// Its safe name.
        name: String,
        /// Its size in bytes.
        size: u64,
    },
    /// Something to continue here: a link (its host) or a file (its name).
    Handoff {
        /// The link's host.
        url_host: Option<String>,
        /// The file's safe name.
        file: Option<String>,
    },
}

/// The asks waiting for an answer.
#[derive(Debug, Default)]
pub(super) struct Asks {
    next: AtomicU64,
    pending: Mutex<HashMap<AskId, mpsc::Sender<Answer>>>,
}

impl Asks {
    /// Ask the user about `kind` from `device` and wait for the answer:
    /// `None` once `expiry` passed or `cancel` fired (the link closed),
    /// then [`Event::AskExpired`].
    pub(super) fn ask(
        &self,
        sink: &dyn EventSink,
        device: DeviceId,
        kind: AskKind,
        expiry: Duration,
        cancel: &Cancel,
    ) -> Option<Answer> {
        let ask = AskId(self.next.fetch_add(1, Ordering::Relaxed));
        let (tx, rx) = mpsc::channel();
        self.lock().insert(ask, tx);
        sink.emit(Event::AskReceived { ask, device, kind });
        let until = Instant::now() + expiry;
        let mut answer = None;
        while answer.is_none() && !cancel.is_cancelled() {
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                break;
            }
            match rx.recv_timeout(left.min(SLICE)) {
                Ok(given) => answer = Some(given),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
        self.lock().remove(&ask);
        // An answer given just as it expired still counts.
        let answer = answer.or_else(|| rx.try_recv().ok());
        if answer.is_none() {
            sink.emit(Event::AskExpired { ask });
        }
        answer
    }

    /// Answer `ask`; [`CoreError::AskGone`] once it expired or was
    /// answered.
    pub(super) fn answer(&self, ask: AskId, answer: Answer) -> Result<()> {
        // Sent under the lock: the asker's own removal then sees it.
        let mut pending = self.lock();
        let tx = pending.remove(&ask).ok_or(CoreError::AskGone)?;
        tx.send(answer).map_err(|_| CoreError::AskGone)
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<AskId, mpsc::Sender<Answer>>> {
        self.pending.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
