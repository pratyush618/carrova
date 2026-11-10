//! [`Outcome`]: what undoing one journal record did (or, in a preview,
//! would do).

use std::path::{Path, PathBuf};

use super::error::RollbackStepError as E;

/// What undoing one record did.
#[derive(Debug, Default)]
pub(super) struct Outcome {
    pub(super) removed: Vec<String>,
    pub(super) restored: Vec<String>,
    pub(super) quarantined: Vec<(String, PathBuf)>,
    pub(super) failed: Vec<(String, E)>,
}

impl Outcome {
    pub(super) fn removed(t: &Path) -> Self {
        Self {
            removed: vec![t.display().to_string()],
            ..Self::default()
        }
    }

    pub(super) fn restored(t: &Path) -> Self {
        Self {
            restored: vec![t.display().to_string()],
            ..Self::default()
        }
    }

    /// The step on `target` failed.
    pub(super) fn failed(target: String, why: E) -> Self {
        Self {
            failed: vec![(target, why)],
            ..Self::default()
        }
    }

    pub(super) fn merge(&mut self, o: Self) {
        self.removed.extend(o.removed);
        self.restored.extend(o.restored);
        self.quarantined.extend(o.quarantined);
        self.failed.extend(o.failed);
    }
}
