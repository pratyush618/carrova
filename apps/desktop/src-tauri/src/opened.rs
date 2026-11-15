//! Links the app was opened with: on a phone, the share sheet (files, as
//! `file:` links to copies, or shared text), the quick-settings tile and
//! the notification's "Send clipboard" (`carrova://send-clipboard`).
//!
//! Plumbing only, like [`crate::send_request`]: the links are queued as
//! text and [`OPENED_EVENT`] tells the UI, which takes them with
//! `take_opened` and decides what to do with each. They are never logged
//! (shared text travels in them).

use std::sync::{Mutex, PoisonError};

/// The Tauri event (no payload) telling the UI that links are queued.
pub const OPENED_EVENT: &str = "carrova://opened";

/// Links waiting for the UI, oldest first.
#[derive(Debug, Default)]
pub struct Opened(Mutex<Vec<String>>);

impl Opened {
    /// Queue `links` after those already waiting.
    pub fn push(&self, links: impl IntoIterator<Item = String>) {
        self.lock().extend(links);
    }

    /// Every queued link, oldest first, emptying the queue.
    #[must_use]
    pub fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.lock())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<String>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "../tests/opened/unit.rs"]
mod tests;
