//! Cancellation of blocking calls.
//!
//! The caller owns the flag (a [`CancelSignal`], e.g. an [`AtomicBool`]
//! shared with another thread) and attaches it through
//! [`crate::ConnectOptions::cancel`] / [`crate::ListenOptions::cancel`];
//! every connection made that way inherits it. Blocking calls poll it on
//! the runtime every [`CANCEL_POLL`] and end with
//! [`TransportError::Cancelled`]. Tokio never leaves this crate: the
//! caller only flips a flag.

use std::fmt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::error::{Result, TransportError};
use crate::runtime::runtime;
use crate::tuning::CANCEL_POLL;

/// A cancellation flag polled by this crate's blocking calls.
pub trait CancelSignal: Send + Sync + fmt::Debug {
    /// Whether the caller asked to stop.
    fn is_cancelled(&self) -> bool;
}

impl CancelSignal for AtomicBool {
    fn is_cancelled(&self) -> bool {
        self.load(Ordering::Acquire)
    }
}

/// The signal a call watches; none = never cancelled.
#[derive(Debug, Clone, Default)]
pub(crate) struct Watch(Option<Arc<dyn CancelSignal>>);

impl Watch {
    /// Watch `signal` (if any).
    pub(crate) fn new(signal: Option<&Arc<dyn CancelSignal>>) -> Self {
        Self(signal.cloned())
    }

    /// Whether the caller asked to stop.
    pub(crate) fn is_cancelled(&self) -> bool {
        self.0.as_ref().is_some_and(|s| s.is_cancelled())
    }

    /// Resolves once the caller asked to stop (never without a signal).
    pub(crate) async fn cancelled(&self) {
        if self.0.is_none() {
            return std::future::pending().await;
        }
        while !self.is_cancelled() {
            tokio::time::sleep(CANCEL_POLL).await;
        }
    }

    /// Drive `fut` on the runtime from the caller's thread, ending with
    /// [`TransportError::Cancelled`] (and dropping `fut`) once the caller
    /// asks to stop. A finished `fut` wins over a simultaneous cancel.
    pub(crate) fn block_on<T>(&self, fut: impl Future<Output = Result<T>>) -> Result<T> {
        runtime()?.block_on(async {
            tokio::select! {
                biased;
                result = fut => result,
                () = self.cancelled() => Err(TransportError::Cancelled),
            }
        })
    }
}

#[cfg(test)]
#[path = "../tests/cancel/unit.rs"]
mod tests;
