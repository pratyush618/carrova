//! Which filesystem a space need is about ([`FsLocation`]) and how far
//! short it is ([`SpaceShortfall`]).

use std::fmt;

use carrova_platform::SemanticRoot;
use serde::{Deserialize, Serialize};

/// A filesystem, named by what lives on it: Carrova's state dir and/or
/// target roots. Displays as `the state dir and user_config, home`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FsLocation {
    /// It holds Carrova's state dir.
    pub state_dir: bool,
    /// Target roots on it.
    pub roots: Vec<SemanticRoot>,
}

impl fmt::Display for FsLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match (self.state_dir, self.roots.is_empty()) {
            (true, true) => return f.write_str("the state dir"),
            (true, false) => f.write_str("the state dir and ")?,
            (false, _) => {}
        }
        for (i, root) in self.roots.iter().enumerate() {
            if i > 0 {
                f.write_str(", ")?;
            }
            f.write_str(root.as_str())?;
        }
        Ok(())
    }
}

/// A filesystem known to be short of space for a plan (a
/// [`crate::plan::Blocker::FreeSpace`], or a warning when overridden).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SpaceShortfall {
    /// [`required_space`](super::required_space) of what lands there.
    pub needed: u64,
    /// Free bytes.
    pub available: u64,
    /// Which filesystem.
    pub location: FsLocation,
}

impl fmt::Display for SpaceShortfall {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} bytes needed on the filesystem of {} (incl. margin), {} available",
            self.needed, self.location, self.available
        )
    }
}
