//! [`SpaceNeed`]: the space check of one filesystem.

use carrova_platform::SemanticRoot;
use serde::{Deserialize, Serialize};

use super::{FsLocation, SpaceShortfall};

/// Space check of one filesystem a plan writes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SpaceNeed {
    /// This filesystem holds Carrova's state dir (staged files, journal,
    /// backups).
    pub state_dir: bool,
    /// Target roots on this filesystem that receive data.
    pub roots: Vec<SemanticRoot>,
    /// Payload bytes landing here (staged and/or final copies).
    pub bytes: u64,
    /// [`required_space`](super::required_space) of `bytes`.
    pub needed: u64,
    /// Free bytes, if they could be determined.
    pub available: Option<u64>,
    /// Capacity of the filesystem, if it could be determined.
    #[serde(default)]
    pub total: Option<u64>,
}

impl SpaceNeed {
    /// Known to be short of space.
    #[must_use]
    pub fn is_short(&self) -> bool {
        self.available.is_some_and(|a| a < self.needed)
    }

    /// `"the state dir and user_config, home"`-style description.
    #[must_use]
    pub fn describe(&self) -> String {
        self.location().to_string()
    }

    /// Which filesystem this is.
    #[must_use]
    pub fn location(&self) -> FsLocation {
        FsLocation {
            state_dir: self.state_dir,
            roots: self.roots.clone(),
        }
    }

    /// How far short of space this filesystem is, if it is known to be.
    #[must_use]
    pub fn shortfall(&self) -> Option<SpaceShortfall> {
        self.is_short().then(|| SpaceShortfall {
            needed: self.needed,
            available: self.available.unwrap_or(0),
            location: self.location(),
        })
    }
}
