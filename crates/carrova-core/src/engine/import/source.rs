//! Where an import's data comes from ([`ImportFrom`]) and its manifest
//! ([`Loaded`]).

use carrova_manifest::Manifest;

use crate::engine::archive::ArchiveSource;
use crate::error::{CoreError, Result};
use crate::lan::Link;
use crate::session::State;

/// Where an import's data comes from.
#[derive(Debug)]
pub(in crate::engine) enum ImportFrom {
    /// Archive file, reopened for staging.
    Archive(ArchiveSource),
    /// LAN sender; `None` once the session ended (sender notified) or
    /// before a resumed session reconnects.
    Lan(Option<Box<Link>>),
}

/// The import's manifest.
#[derive(Debug)]
pub(in crate::engine) enum Loaded {
    /// Planning and applying work from it.
    Open(Manifest),
    /// The session was finished at resume (an interrupted apply was rolled
    /// back); the manifest is kept for display if it could still be read.
    Finished(Option<Manifest>),
}

impl Loaded {
    pub(super) fn get(&self) -> Option<&Manifest> {
        match self {
            Self::Open(m) => Some(m),
            Self::Finished(m) => m.as_ref(),
        }
    }

    /// The manifest for a session in `from` to move on to `to`; a session
    /// finished at resume cannot move on.
    pub(in crate::engine) fn open(&self, from: State, to: State) -> Result<&Manifest> {
        match self {
            Self::Open(manifest) => Ok(manifest),
            Self::Finished(_) => Err(CoreError::InvalidTransition { from, to }),
        }
    }
}
