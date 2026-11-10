use std::fmt;

use serde::{Deserialize, Serialize};

/// Unique session id: `YYYYMMDD-HHMMSS-<6 hex>` (sortable, filesystem-safe).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(transparent)]
pub struct SessionId(pub String);

impl SessionId {
    /// Whether `s` has the id shape (used before joining it into a path).
    #[must_use]
    pub fn is_valid(s: &str) -> bool {
        !s.is_empty()
            && s.len() <= 64
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which side of a migration this session is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Export,
    Import,
}

impl Role {
    /// Name shown to users (`export`, `import`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Export => "export",
            Self::Import => "import",
        }
    }
}

/// Renders [`Role::as_str`] (honours width/alignment).
impl fmt::Display for Role {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

/// Session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum State {
    Scanned,
    Selected,
    Planned,
    Transferring,
    Staged,
    Applying,
    Done,
    RolledBack,
    Failed,
}

impl State {
    /// Allowed transitions (same state = no-op allowed, for resume).
    #[must_use]
    pub fn can_transition(self, to: Self) -> bool {
        use State::{
            Applying, Done, Failed, Planned, RolledBack, Scanned, Selected, Staged, Transferring,
        };
        if self == to {
            return true;
        }
        matches!(
            (self, to),
            (Scanned, Selected)
                | (Selected, Scanned | Planned | Transferring)
                | (Planned, Selected | Transferring)
                // Transferring → Planned: resume after an interrupted stage.
                // Transferring → Selected: a LAN send that ended before any
                // receiver paired (nothing left the machine).
                | (Transferring, Staged | Done | Planned | Selected)
                | (Staged, Applying | Planned)
                | (Applying, Done | RolledBack)
                | (Done, RolledBack)
                | (
                    Scanned | Selected | Planned | Transferring | Staged | Applying,
                    Failed
                )
        )
    }

    /// Name shown to users, e.g. `rolled back`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scanned => "scanned",
            Self::Selected => "selected",
            Self::Planned => "planned",
            Self::Transferring => "transferring",
            Self::Staged => "staged",
            Self::Applying => "applying",
            Self::Done => "done",
            Self::RolledBack => "rolled back",
            Self::Failed => "failed",
        }
    }

    /// No further transitions except manual rollback of `Done`.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Done | Self::RolledBack | Self::Failed)
    }
}

/// Renders [`State::as_str`] (honours width/alignment).
impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(self.as_str())
    }
}

#[cfg(test)]
#[path = "../../tests/session/state/unit.rs"]
mod tests;
