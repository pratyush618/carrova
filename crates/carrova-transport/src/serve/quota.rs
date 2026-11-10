//! Per-connection ceiling on requested chunk bytes.

use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::error::{ProtocolViolation, Result};

/// Chunk bytes one connection's receiver has requested, against its
/// ceiling ([`SERVED_BYTES_FACTOR`] × the allowed chunks' unique bytes).
///
/// [`SERVED_BYTES_FACTOR`]: crate::tuning::SERVED_BYTES_FACTOR
#[derive(Debug, Clone)]
pub(super) struct Quota {
    pub(super) requested: Arc<AtomicU64>,
    pub(super) ceiling: u64,
}

impl Quota {
    /// Account for one more requested chunk of `len` bytes.
    pub(super) fn take(&self, len: u32) -> Result<()> {
        let before = self.requested.fetch_add(u64::from(len), Ordering::Relaxed);
        if before.saturating_add(u64::from(len)) > self.ceiling {
            return Err(ProtocolViolation::OverRequested.into());
        }
        Ok(())
    }
}
