//! Assets from a local directory (offline bundles, shipped resources,
//! tests).

use std::fs::File;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use super::{Fetcher, read_capped};
use crate::{Asset, FetchError};

/// Reads assets from a bundle directory (the layout `cargo xtask bundle
/// build` writes).
#[derive(Debug, Clone)]
pub struct DirFetcher {
    dir: PathBuf,
}

impl DirFetcher {
    /// Fetch from `dir`.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    /// The directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }
}

impl Fetcher for DirFetcher {
    fn get(&self, asset: &Asset, limit: u64) -> Result<Vec<u8>, FetchError> {
        let file = File::open(self.dir.join(asset.file_name())).map_err(|e| match e.kind() {
            ErrorKind::NotFound => FetchError::NotFound,
            _ => FetchError::Io(e),
        })?;
        read_capped(file, limit)
    }
}
