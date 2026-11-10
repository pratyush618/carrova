//! Cooperative cancellation of a running session call.
//!
//! # Design
//! Every session object owns one [`Cancel`] and hands out clones
//! ([`crate::Exporter::canceller`], [`crate::Importer::canceller`],
//! [`crate::SendHandle::canceller`]). A front-end takes the clone *before*
//! the blocking call and triggers it from another thread. Calls that
//! create the object take the flag up front instead
//! ([`crate::Exporter::start_with`], [`crate::Importer::connect_with`],
//! [`crate::Importer::resume_lan_with`]); it becomes the object's own.
//! The engine checks it between recipes and entries (scan), items, files
//! and chunks (capture, archive write, staging, LAN transfer) and inside
//! every blocking LAN wait (discovery, pairing, accept, receive, fetch,
//! serve, reconnect), so a call returns [`CoreError::Cancelled`] promptly.
//!
//! # Semantics
//! - **Sticky:** once cancelled, the object's further blocking calls fail
//!   with [`CoreError::Cancelled`] at once. To continue, resume the
//!   session with a new object ([`crate::Importer::resume_lan`],
//!   [`crate::Importer::resume_archive`], [`crate::Exporter::resume_send`])
//!   or dismiss it. One exception: a LAN send cancelled before any
//!   receiver paired returns to its selection, and
//!   [`crate::Exporter::reopen_after_send`] re-arms the exporter.
//! - **Never mid-apply:** once apply starts writing to the user's roots
//!   it runs to completion or rolls back; a cancel arriving meanwhile is
//!   ignored for that apply. A cancel before apply starts leaves the
//!   user's roots untouched.
//! - **What remains:** a cancelled archive export removes its partial
//!   archive (session `Failed`). A cancelled import goes back to
//!   `Planned` with its approval (LAN staging kept, so a resume fetches
//!   only what is missing). A cancelled LAN send stays resumable once
//!   paired (back at `Selected` if no receiver ever paired).
//! - **The peer is told:** a LAN side that cancels sends
//!   [`crate::lan::AbortReason::Cancelled`] and closes the connection as
//!   cancelled; the peer's call fails with [`CoreError::PeerCancelled`].

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use carrova_manifest::{ChunkRef, ChunkSource, ChunkSourceError};
use carrova_transport::CancelSignal;

use crate::error::{CoreError, Result};

/// Longest a [`Cancel::sleep`] waits between checks.
const SLEEP_SLICE: Duration = Duration::from_millis(50);

/// Cancellation flag of one session object. Cheap to clone; every clone
/// shares the flag. See the [module docs](self) for the semantics.
#[derive(Debug, Clone, Default)]
pub struct Cancel(Arc<AtomicBool>);

impl Cancel {
    /// Ask the running (or next) blocking call to stop.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }

    /// Whether [`Self::cancel`] was called.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }

    /// [`CoreError::Cancelled`] once cancelled.
    pub fn check(&self) -> Result<()> {
        if self.is_cancelled() {
            return Err(CoreError::Cancelled);
        }
        Ok(())
    }

    /// Clear the flag so the object can run again. Only the engine does
    /// this, and only where the cancelled call has returned and a new one
    /// is allowed ([`crate::Exporter::reopen_after_send`]).
    pub(crate) fn reset(&self) {
        self.0.store(false, Ordering::Release);
    }

    /// Wait `duration`, ending early with [`CoreError::Cancelled`].
    pub(crate) fn sleep(&self, duration: Duration) -> Result<()> {
        let until = Instant::now() + duration;
        loop {
            self.check()?;
            let left = until.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Ok(());
            }
            std::thread::sleep(left.min(SLEEP_SLICE));
        }
    }

    /// The flag as the transport's cancel signal.
    pub(crate) fn signal(&self) -> Arc<dyn CancelSignal> {
        self.0.clone()
    }
}

/// A [`ChunkSource`] that stops with [`ChunkSourceError::Cancelled`] once
/// its [`Cancel`] fires (checked before every chunk).
#[derive(Debug)]
pub(crate) struct Cancellable<'c, S> {
    inner: S,
    cancel: &'c Cancel,
}

impl<'c, S: ChunkSource> Cancellable<'c, S> {
    /// Wrap `inner`, watching `cancel`.
    pub(crate) fn new(inner: S, cancel: &'c Cancel) -> Self {
        Self { inner, cancel }
    }
}

impl<S: ChunkSource> ChunkSource for Cancellable<'_, S> {
    fn read_chunk(&mut self, chunk: &ChunkRef) -> Result<Vec<u8>, ChunkSourceError> {
        if self.cancel.is_cancelled() {
            return Err(ChunkSourceError::Cancelled);
        }
        self.inner.read_chunk(chunk)
    }
}

#[cfg(test)]
#[path = "../tests/cancel/unit.rs"]
mod tests;
