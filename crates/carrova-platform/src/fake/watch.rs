//! Pretend directory watches for [`FakePlatform`]: nothing watches the
//! disk; a test reports a change with [`FakePlatform::fire_dir_change`].

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::FakePlatform;
use crate::{DirWatch, PlatformError};

/// One live watch: its dirs and callback.
type Watch = (u64, Vec<PathBuf>, Box<dyn Fn() + Send>);

/// Live watches, shared by clones like the hooks.
#[derive(Default)]
pub(super) struct FakeWatches {
    next: AtomicU64,
    live: Mutex<Vec<Watch>>,
}

impl fmt::Debug for FakeWatches {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FakeWatches")
            .field("live", &self.lock().len())
            .finish()
    }
}

impl FakeWatches {
    fn lock(&self) -> MutexGuard<'_, Vec<Watch>> {
        self.live.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl FakePlatform {
    /// [`crate::Platform::watch_dirs`]: registers `changed` until the
    /// guard drops (unsupported under [`super::Hooks::unwatched`]).
    pub(super) fn watch(
        &self,
        dirs: &[PathBuf],
        changed: Box<dyn Fn() + Send>,
    ) -> Result<DirWatch, PlatformError> {
        if self.hooks.hooks.unwatched {
            return Err(PlatformError::Unsupported("watch_dirs"));
        }
        let watches = Arc::clone(&self.watches);
        let id = watches.next.fetch_add(1, Ordering::SeqCst);
        watches.lock().push((id, dirs.to_vec(), changed));
        Ok(DirWatch::new(move || {
            watches.lock().retain(|(live, ..)| *live != id);
        }))
    }

    /// Pretend an entry of `dir` changed: calls back every live watch of
    /// `dir`, on this thread.
    pub fn fire_dir_change(&self, dir: &Path) {
        for (_, dirs, changed) in self.watches.lock().iter() {
            if dirs.iter().any(|d| d == dir) {
                changed();
            }
        }
    }
}
