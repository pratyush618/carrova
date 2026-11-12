//! Pretend directory watches for [`FakePlatform`]: nothing watches the
//! disk; a test reports a change with [`FakePlatform::fire_dir_change`].

use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use super::FakePlatform;
use crate::{DirWatch, PlatformError};

/// A watch's callback; `None` once its guard dropped. Called under its own
/// lock, never the registry's, so a callback may change the watches.
type Callback = Arc<Mutex<Option<Box<dyn Fn() + Send>>>>;

/// One live watch: its dirs and callback.
type Watch = (u64, Vec<PathBuf>, Callback);

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
        let callback: Callback = Arc::new(Mutex::new(Some(changed)));
        watches
            .lock()
            .push((id, dirs.to_vec(), Arc::clone(&callback)));
        Ok(DirWatch::new(move || {
            watches.lock().retain(|(live, ..)| *live != id);
            // Waits for a call in progress: none comes after the drop.
            locked(&callback).take();
        }))
    }

    /// Pretend every live watch of `dir` failed: each ends by itself and
    /// drops its callback, as a real one does when its watcher fails.
    pub fn end_dir_watches(&self, dir: &Path) {
        let mut ended = Vec::new();
        self.watches.lock().retain(|(_, dirs, changed)| {
            let ends = dirs.iter().any(|d| d == dir);
            if ends {
                ended.push(Arc::clone(changed));
            }
            !ends
        });
        for changed in ended {
            locked(&changed).take();
        }
    }

    /// Pretend an entry of `dir` changed: calls back every live watch of
    /// `dir`, on this thread.
    pub fn fire_dir_change(&self, dir: &Path) {
        let due: Vec<Callback> = self
            .watches
            .lock()
            .iter()
            .filter(|(_, dirs, _)| dirs.iter().any(|d| d == dir))
            .map(|(.., changed)| Arc::clone(changed))
            .collect();
        for changed in due {
            if let Some(changed) = locked(&changed).as_ref() {
                changed();
            }
        }
    }
}

fn locked(callback: &Callback) -> MutexGuard<'_, Option<Box<dyn Fn() + Send>>> {
    callback.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
#[path = "../../tests/fake/watch/unit.rs"]
mod tests;
