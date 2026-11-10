//! A [`ReportFs`] over `std::fs` that records which files it created
//! privately. Permission bits are the front-ends' job (their `Platform`
//! adapter is tested for `0600`); here only the routing is checked.

use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use carrova_report::{ReportFs, ReportStore};

/// `std::fs` without permission bits, remembering private creations.
#[derive(Debug, Default)]
pub struct RecordingFs {
    created: Mutex<Vec<PathBuf>>,
}

impl RecordingFs {
    /// Final names of the files created through [`ReportFs::create_private`]
    /// and then moved into place (temporary names resolved by `replace`).
    pub fn private_files(&self) -> Vec<PathBuf> {
        self.created
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl ReportFs for RecordingFs {
    fn create_private(&self, path: &Path) -> io::Result<File> {
        OpenOptions::new().write(true).create_new(true).open(path)
    }

    fn replace(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)?;
        self.created
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(to.to_path_buf());
        Ok(())
    }
}

/// A store under `state_dir` with a fresh [`RecordingFs`].
pub fn store_in(state_dir: &Path) -> (ReportStore, Arc<RecordingFs>) {
    let fs = Arc::new(RecordingFs::default());
    (ReportStore::in_state_dir(state_dir, fs.clone()), fs)
}
