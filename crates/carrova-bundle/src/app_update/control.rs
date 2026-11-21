//! The user's wishes for the update in progress, of which there is at
//! most one.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use carrova_platform::UpdateTap;

use super::AppUpdateError;

/// What the user asked of the update in progress.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wish {
    /// Hold the download where it is.
    pub paused: bool,
    /// Stop: nothing is installed.
    pub cancelled: bool,
    /// Download on mobile data too (for this update).
    pub mobile_data: bool,
}

/// Carries the user's wishes to the running update; a held update wakes
/// at once when one changes.
#[derive(Debug, Default)]
pub struct Control {
    wish: Mutex<Wish>,
    changed: Condvar,
    running: AtomicBool,
}

/// The update in progress; the next may start once this drops.
#[derive(Debug)]
pub struct Running<'a>(&'a Control);

impl Control {
    /// Start an update with fresh wishes (`mobile_data` for this one).
    ///
    /// # Errors
    ///
    /// [`AppUpdateError::Busy`] while another runs; its wishes stay.
    pub fn begin(&self, mobile_data: bool) -> Result<Running<'_>, AppUpdateError> {
        if self.running.swap(true, Ordering::AcqRel) {
            return Err(AppUpdateError::Busy);
        }
        self.update(|wish| {
            *wish = Wish {
                mobile_data,
                ..Wish::default()
            };
        });
        Ok(Running(self))
    }

    /// The user's wishes now.
    pub fn wish(&self) -> Wish {
        *self.lock()
    }

    /// Change the wishes with `change` and wake a held update.
    pub fn update(&self, change: impl FnOnce(&mut Wish)) {
        change(&mut self.lock());
        self.changed.notify_all();
    }

    /// Apply a tap on the notification's actions.
    pub(super) fn tap(&self, tap: UpdateTap) {
        self.update(|wish| match tap {
            UpdateTap::Pause => wish.paused = true,
            UpdateTap::Resume => wish.paused = false,
            UpdateTap::Cancel => wish.cancelled = true,
        });
    }

    /// Wait up to `timeout` for a change.
    pub(super) fn wait(&self, timeout: Duration) {
        let guard = self.lock();
        // Spurious wake-ups and timeouts alike just look again.
        drop(self.changed.wait_timeout(guard, timeout));
    }

    fn lock(&self) -> MutexGuard<'_, Wish> {
        self.wish.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Drop for Running<'_> {
    fn drop(&mut self) {
        self.0.running.store(false, Ordering::Release);
    }
}
