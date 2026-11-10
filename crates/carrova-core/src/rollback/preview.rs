//! [`preview`]: what undoing an import would do, without doing it.

use std::path::Path;

use carrova_platform::Platform;
use serde::{Deserialize, Serialize};

use super::effects::Fs;
use super::report::{RollbackFailure, RollbackReport};
use super::{Run, read_journal};
use crate::error::Result;
use crate::session::NullSink;

/// What a rollback would do now: counts per outcome plus the steps it
/// would have to leave for manual attention. Computed by running the
/// real undo logic against a read-only view of the disk, so it matches
/// what [`crate::sessions::rollback`] would report if nothing changes meanwhile.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RollbackPreview {
    /// Paths whose pre-import version would come back.
    pub restored: u32,
    /// Paths the import created that would be deleted again.
    pub removed: u32,
    /// Paths that would be moved aside (quarantined) instead of deleted.
    pub moved_aside: u32,
    /// Steps that could not be undone (changed since the import, location
    /// occupied, …): the user resolves them by hand.
    pub blockers: Vec<RollbackFailure>,
}

impl From<RollbackReport> for RollbackPreview {
    fn from(report: RollbackReport) -> Self {
        let count = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        Self {
            restored: count(report.restored.len()),
            removed: count(report.removed.len()),
            moved_aside: count(report.quarantined.len()),
            blockers: report.failed,
        }
    }
}

/// Preview undoing the apply recorded in `session_dir/journal.cbor`:
/// nothing is written, moved or marked, and no events are emitted.
pub fn preview(platform: &dyn Platform, session_dir: &Path) -> Result<RollbackPreview> {
    let records = read_journal(platform, session_dir)?;
    let mut run = Run::new(
        Fs::preview(platform),
        session_dir,
        &records,
        None,
        &NullSink,
    );
    run.undo_all(&records)?;
    Ok(run.report.into())
}
