//! The user's live controls over a running LAN transfer: pause/resume,
//! and the "continue anyway" override of the low-disk-space pause.
//!
//! # Design
//! Like [`Cancel`], an [`Importer`](crate::Importer) owns one
//! [`TransferControl`] and hands out clones
//! ([`crate::Importer::transfer_control`]); a front-end takes the clone
//! before the blocking `apply` and flips it from another thread.
//!
//! # Semantics
//! - **Pause** holds the transfer between batches (the batch in flight
//!   finishes and is checkpointed). The connection is kept; the sender
//!   simply waits. [`Event::TransferPaused`] with [`PauseReason::ByUser`]
//!   is emitted once, [`Event::TransferResumed`] when the user resumes. A
//!   cancel ends a paused transfer as usual.
//! - **Ignore free space** skips the transfer's free-space guard
//!   ([`crate::lan::check_space`]) from now on, ending a low-disk-space
//!   pause within [`IDLE_SLICE`]. A disk that is really full still stops
//!   the transfer: the safety margin is advisory, a failed write is not.
//!   Sticky for the importer's lifetime.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::cancel::Cancel;
use crate::error::Result;
use crate::session::{Event, EventSink, PauseReason};

/// How often a paused transfer re-checks its controls (and the cancel).
pub const IDLE_SLICE: Duration = Duration::from_millis(100);

#[derive(Debug, Default)]
struct Flags {
    paused: AtomicBool,
    ignore_free_space: AtomicBool,
}

/// Live controls of one import's LAN transfer. Cheap to clone; every
/// clone shares the flags. See the [module docs](self).
#[derive(Debug, Clone, Default)]
pub struct TransferControl(Arc<Flags>);

impl TransferControl {
    /// Hold the transfer after the batch in flight.
    pub fn pause(&self) {
        self.0.paused.store(true, Ordering::Release);
    }

    /// Continue a transfer held by [`Self::pause`].
    pub fn resume(&self) {
        self.0.paused.store(false, Ordering::Release);
    }

    /// Whether the user holds the transfer.
    #[must_use]
    pub fn is_paused(&self) -> bool {
        self.0.paused.load(Ordering::Acquire)
    }

    /// Continue despite low free space (the user's override, given after
    /// the warning). Sticky.
    pub fn ignore_free_space(&self) {
        self.0.ignore_free_space.store(true, Ordering::Release);
    }

    /// Whether the free-space guard is overridden.
    #[must_use]
    pub fn ignores_free_space(&self) -> bool {
        self.0.ignore_free_space.load(Ordering::Acquire)
    }

    /// Block while the user holds the transfer, announcing the pause and
    /// the resumption on `sink`; `cancel` ends the wait with
    /// [`crate::CoreError::Cancelled`].
    pub(crate) fn hold_while_paused(&self, cancel: &Cancel, sink: &dyn EventSink) -> Result<()> {
        if !self.is_paused() {
            return Ok(());
        }
        tracing::info!("transfer paused by the user");
        sink.emit(Event::TransferPaused {
            reason: PauseReason::ByUser,
        });
        while self.is_paused() {
            cancel.sleep(IDLE_SLICE)?;
        }
        tracing::info!("transfer resumed by the user");
        sink.emit(Event::TransferResumed);
        Ok(())
    }
}

#[cfg(test)]
#[path = "../tests/transfer_control/unit.rs"]
mod tests;
