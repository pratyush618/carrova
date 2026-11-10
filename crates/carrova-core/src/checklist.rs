//! End-of-session checklist: what moved, what didn't (and why), what to install.

use serde::{Deserialize, Serialize};

use crate::key::ItemKey;

/// Shown when a session finishes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Checklist {
    /// Item keys moved (or exported).
    pub moved: Vec<ItemKey>,
    /// Item keys or paths not moved, with a human-readable reason.
    pub skipped: Vec<Skipped>,
    /// Apps whose data arrived but which are not installed here
    /// (recipe name + detect binaries as a hint).
    pub apps_to_install: Vec<String>,
    /// A rollback point exists until the user dismisses it.
    pub rollback_available: bool,
}

/// One thing that did not move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Skipped {
    pub what: String,
    pub reason: String,
}
