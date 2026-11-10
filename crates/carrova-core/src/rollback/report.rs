//! What a rollback reports: [`RollbackReport`] and its parts.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::outcome::Outcome;

/// Outcome of a rollback.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RollbackReport {
    /// Paths whose pre-import version is back.
    pub restored: Vec<String>,
    /// Paths the import created, deleted again.
    pub removed: Vec<String>,
    /// Steps that could not be undone (manual attention).
    pub failed: Vec<RollbackFailure>,
    /// Paths moved out of the way instead of deleted.
    pub quarantined: Vec<Quarantined>,
}

/// One rollback step that could not be undone ([`RollbackReport::failed`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RollbackFailure {
    /// The path concerned.
    pub target: String,
    /// Why it could not be undone.
    pub reason: String,
}

/// One path moved aside instead of deleted ([`RollbackReport::quarantined`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Quarantined {
    /// The original path.
    pub target: String,
    /// Where it is now, in the session's `rollback-trash/`.
    pub moved_to: PathBuf,
}

impl RollbackReport {
    /// `target: why` of every failed step, `; `-separated (the text
    /// session records and errors carry).
    #[must_use]
    pub fn failure_summary(&self) -> String {
        self.failed
            .iter()
            .map(|f| format!("{}: {}", f.target, f.reason))
            .collect::<Vec<_>>()
            .join("; ")
    }

    pub(super) fn merge(&mut self, out: Outcome) {
        self.removed.extend(out.removed);
        self.restored.extend(out.restored);
        self.quarantined.extend(
            out.quarantined
                .into_iter()
                .map(|(target, moved_to)| Quarantined { target, moved_to }),
        );
        self.failed
            .extend(out.failed.into_iter().map(|(target, why)| RollbackFailure {
                target,
                reason: why.to_string(),
            }));
    }
}
