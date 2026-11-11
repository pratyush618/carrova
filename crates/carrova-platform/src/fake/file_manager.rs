//! Pretend file-manager entries for [`FakePlatform`](super::FakePlatform):
//! nothing is written, every install and remove is recorded.

use std::sync::{Mutex, PoisonError};

use crate::FileManagerStatus;

/// A write to the fake's file-manager entries
/// ([`FakePlatform::entries_writes`](super::FakePlatform::entries_writes)).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntriesWrite {
    /// [`Platform::install_file_manager_entries`](crate::Platform::install_file_manager_entries).
    Install,
    /// [`Platform::remove_file_manager_entries`](crate::Platform::remove_file_manager_entries).
    Remove,
}

/// The entries' state: installed at start, then every write in call order.
#[derive(Debug)]
pub(super) struct FakeEntries {
    installed: bool,
    writes: Mutex<Vec<EntriesWrite>>,
}

impl FakeEntries {
    pub(super) fn new(installed: bool) -> Self {
        Self {
            installed,
            writes: Mutex::default(),
        }
    }

    pub(super) fn status(&self) -> FileManagerStatus {
        let last = self.writes().last().copied();
        FileManagerStatus {
            supported: true,
            installed: last.map_or(self.installed, |w| w == EntriesWrite::Install),
            skipped: Vec::new(),
        }
    }

    pub(super) fn write(&self, write: EntriesWrite) -> FileManagerStatus {
        self.lock().push(write);
        self.status()
    }

    pub(super) fn writes(&self) -> Vec<EntriesWrite> {
        self.lock().clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<EntriesWrite>> {
        self.writes.lock().unwrap_or_else(PoisonError::into_inner)
    }
}
