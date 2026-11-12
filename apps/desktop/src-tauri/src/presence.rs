//! This computer's presence on the LAN while the app runs (in the window
//! or the tray): the engine's [`Presence`] under the visibility saved in
//! the state dir, the one value the CLI's `carrova visibility` also sets.

use std::sync::{Arc, Mutex, PoisonError};

use carrova_core::presence::Presence;
use carrova_core::{Engine, NullSink, Recipe};
use carrova_platform::Platform;

/// Where the bridge's changes to visibility and known devices reach the
/// advert (a seam: tests record the calls).
pub trait Advertise: Send + Sync {
    /// Advertise under the saved visibility with the known devices as they
    /// are now (after a visibility change, or a device remembered or
    /// forgotten).
    fn refresh(&self);
}

/// The app's advert: nothing until [`Self::start`], then the engine's
/// [`Presence`] until [`Self::stop`] (for good: a start after it does
/// nothing, so a start racing app exit never outlives it).
pub struct BackgroundPresence {
    platform: Arc<dyn Platform>,
    running: Mutex<Running>,
}

/// Where the advert is in its life.
enum Running {
    NotYet,
    Now(Presence),
    Stopped,
}

impl std::fmt::Debug for BackgroundPresence {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BackgroundPresence").finish_non_exhaustive()
    }
}

impl BackgroundPresence {
    /// Not advertising yet, on `platform`.
    pub fn new(platform: Arc<dyn Platform>) -> Self {
        Self {
            platform,
            running: Mutex::new(Running::NotYet),
        }
    }

    /// Start advertising under the saved visibility, unless already
    /// started or stopped.
    pub fn start(&self) {
        let mut running = self.lock();
        if !matches!(*running, Running::NotYet) {
            return;
        }
        let engine = Engine::new(
            Arc::clone(&self.platform),
            Vec::<Recipe>::new(),
            Arc::new(NullSink),
        );
        *running = Running::Now(Presence::start(&engine));
    }

    /// Withdraw the advert for good (blocking until it is gone).
    pub fn stop(&self) {
        let running = std::mem::replace(&mut *self.lock(), Running::Stopped);
        if let Running::Now(presence) = running {
            presence.stop();
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Running> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Advertise for BackgroundPresence {
    fn refresh(&self) {
        if let Running::Now(presence) = &*self.lock() {
            presence.refresh();
        }
    }
}

#[cfg(test)]
#[path = "../tests/presence/unit.rs"]
mod tests;
