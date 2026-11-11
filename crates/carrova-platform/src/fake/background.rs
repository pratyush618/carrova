//! Pretend background integration for [`FakePlatform`]: a status tray a
//! test can take away, and an autostart entry kept in memory.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, PoisonError};

use super::FakePlatform;

#[cfg(test)]
#[path = "../../tests/fake/background/unit.rs"]
mod tests;

/// Shared by clones of one fake, like the hooks.
#[derive(Debug)]
pub(super) struct FakeBackground {
    tray: AtomicBool,
    autostart: Mutex<Option<PathBuf>>,
}

impl Default for FakeBackground {
    fn default() -> Self {
        Self {
            tray: AtomicBool::new(true),
            autostart: Mutex::default(),
        }
    }
}

impl FakeBackground {
    pub(super) fn tray(&self) -> bool {
        self.tray.load(Ordering::SeqCst)
    }

    pub(super) fn autostart(&self) -> Option<PathBuf> {
        self.lock().clone()
    }

    pub(super) fn set_autostart(&self, launcher: &Path, on: bool) -> bool {
        *self.lock() = on.then(|| launcher.to_path_buf());
        on
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Option<PathBuf>> {
        self.autostart
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

impl FakePlatform {
    /// Pretend the desktop shows (`true`, the default) or has no status
    /// tray ([`Platform::has_status_tray`](crate::Platform::has_status_tray));
    /// changeable while the platform is in use.
    pub fn set_status_tray(&self, shown: bool) {
        self.background.tray.store(shown, Ordering::SeqCst);
    }

    /// The launcher the pretend autostart entry starts; `None` while off.
    #[must_use]
    pub fn autostart_launcher(&self) -> Option<PathBuf> {
        self.background.autostart()
    }
}
