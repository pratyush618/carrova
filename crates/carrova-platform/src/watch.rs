//! [`DirWatch`]: the guard returned by [`crate::Platform::watch_dirs`].

use std::fmt;

/// Directories being watched for changes, until the guard is dropped.
/// Dropping it waits for the watcher to stop, so the change callback is
/// never called afterwards (and is itself dropped).
#[must_use = "the watch stops as soon as the guard is dropped"]
pub struct DirWatch {
    stop: Option<Box<dyn FnOnce() + Send>>,
}

impl DirWatch {
    /// Guard that runs `stop` (end the watcher, wait for it) on drop.
    pub(crate) fn new(stop: impl FnOnce() + Send + 'static) -> Self {
        Self {
            stop: Some(Box::new(stop)),
        }
    }
}

impl Drop for DirWatch {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop();
        }
    }
}

impl fmt::Debug for DirWatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DirWatch").finish_non_exhaustive()
    }
}
