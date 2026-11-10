//! On-disk layout of the store and its state pointer.

use std::collections::BTreeSet;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::fsio::Fs;
use crate::{Asset, BundleError, BundleVersion, Result};

/// Store root relative to the user-data semantic root.
const STORE_DIR: [&str; 2] = ["carrova", "recipes"];
/// Largest accepted `state.json` (room for many refused versions).
const STATE_MAX: u64 = 64 * 1024;

/// Paths inside the store root.
#[derive(Debug, Clone)]
pub(super) struct Layout {
    /// The user-data semantic root the store lives under.
    base: PathBuf,
    /// `<base>/carrova/recipes`.
    pub(super) root: PathBuf,
}

impl Layout {
    /// Layout of the store under the user-data root `base`.
    pub(super) fn under(base: PathBuf) -> Self {
        let root = STORE_DIR.iter().fold(base.clone(), |p, part| p.join(part));
        Self { base, root }
    }

    /// Content-addressed blobs (`blobs/<hash>.<ext>`).
    pub(super) fn blobs(&self) -> PathBuf {
        self.root.join("blobs")
    }

    /// One directory per kept index version (`indexes/<n>/`).
    pub(super) fn indexes(&self) -> PathBuf {
        self.root.join("indexes")
    }

    /// Temp files and staging directories.
    pub(super) fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }

    /// The advisory lock every state change holds (`lock`; never pruned
    /// or removed, so all lockers always agree on the same file).
    pub(super) fn lock(&self) -> PathBuf {
        self.root.join("lock")
    }

    /// The state pointer (`state.json`), replaced atomically.
    pub(super) fn state(&self) -> PathBuf {
        self.root.join("state.json")
    }

    /// Directory of index `version`.
    pub(super) fn index_dir(&self, version: BundleVersion) -> PathBuf {
        self.indexes().join(version.0.to_string())
    }

    /// Path of a blob.
    pub(super) fn blob(&self, asset: &Asset) -> PathBuf {
        self.blobs().join(asset.file_name())
    }

    /// Path of `asset` (index or signature) of index `version`.
    pub(super) fn index_file(&self, version: BundleVersion, asset: &Asset) -> PathBuf {
        self.index_dir(version).join(asset.file_name())
    }

    /// Create the store directories (and missing ancestors of the
    /// user-data root). Directories Carrova owns must be real directories.
    pub(super) fn create(&self, fs: Fs<'_>) -> Result<()> {
        fs.create_ancestors(&self.base)?;
        let mut dir = self.base.clone();
        for part in STORE_DIR {
            dir.push(part);
            fs.ensure_dir(&dir)?;
        }
        for sub in [self.blobs(), self.indexes(), self.tmp()] {
            fs.ensure_dir(&sub)?;
        }
        Ok(())
    }
}

/// Which index versions are live, and which the user refused. Switching
/// versions = replacing this one small file atomically, so a crash leaves
/// either the old or new state.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct StoreState {
    /// Installed version (`None` after a reset).
    pub(super) current: Option<BundleVersion>,
    /// Version kept for rollback.
    pub(super) previous: Option<BundleVersion>,
    /// Versions the user rolled back: never offered or installed again.
    #[serde(default)]
    pub(super) refused: BTreeSet<BundleVersion>,
}

impl StoreState {
    /// Read the state; the default (nothing installed, nothing refused)
    /// if there is none yet.
    pub(super) fn read(fs: Fs<'_>, layout: &Layout) -> Result<Self> {
        let path = layout.state();
        if !fs.exists(&path)? {
            return Ok(Self::default());
        }
        let bytes = fs.read_capped(&path, STATE_MAX)?;
        serde_json::from_slice(&bytes).map_err(BundleError::json("store state"))
    }

    /// Replace the state atomically. Callers hold the store lock.
    pub(super) fn write(&self, fs: Fs<'_>, layout: &Layout) -> Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(BundleError::json("store state"))?;
        fs.write_atomic(&layout.tmp(), &layout.state(), &bytes)
    }

    /// Versions whose indexes (and blobs) must be kept.
    pub(super) fn live(&self) -> impl Iterator<Item = BundleVersion> {
        self.current.into_iter().chain(self.previous)
    }
}
