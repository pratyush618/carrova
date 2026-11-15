//! Links the app was opened with: on a phone, the share sheet (files, as
//! `file:` links to copies, or shared text), the quick-settings tile and
//! the notification's "Send clipboard" (`carrova://send-clipboard`).
//!
//! Any app may open Carrova with any link, so only [`admitted`] ones are
//! queued; the UI is told by [`OPENED_EVENT`], takes them with
//! `take_opened` and decides what to do with each. They are never logged
//! (shared text travels in them).

use std::path::{Component, Path};
use std::sync::{Mutex, PoisonError};

use tauri::Url;

/// The Tauri event (no payload) telling the UI that links are queued.
pub const OPENED_EVENT: &str = "carrova://opened";

/// The link asking the UI to send the clipboard (tile, notification).
pub const SEND_CLIPBOARD: &str = "carrova://send-clipboard";

/// Where the share sheet's copies go, under the cache root (the Kotlin
/// `ShareActivity` writes `files/cache/shared`, the Android cache root).
pub const SHARED_DIR: &str = "shared";

/// Whether `link` may reach the UI: [`SEND_CLIPBOARD`], shared text
/// (`http(s)` or `data:text/plain`), or a `file:` link to a share copy
/// strictly inside `shared` (no `..`). Never another file: the app could
/// read its own private ones (its keys) and send them away.
#[must_use]
pub fn admitted(link: &Url, shared: Option<&Path>) -> bool {
    match link.scheme() {
        "http" | "https" => true,
        "data" => link.path().starts_with("text/plain,"),
        "carrova" => link.as_str() == SEND_CLIPBOARD,
        "file" => shared.is_some_and(|shared| {
            link.to_file_path().is_ok_and(|path| {
                path.components()
                    .all(|c| matches!(c, Component::RootDir | Component::Normal(_)))
                    && path.starts_with(shared)
                    && path != shared
            })
        }),
        _ => false,
    }
}

/// Admitted links waiting for the UI, in arrival order (one intent's
/// links arrive in no set order).
#[derive(Debug, Default)]
pub struct Opened(Mutex<Vec<String>>);

impl Opened {
    /// Queue the [`admitted`] `links` (share copies inside `shared`);
    /// returns how many were refused.
    pub fn push_admitted<'a>(
        &self,
        links: impl IntoIterator<Item = &'a Url>,
        shared: Option<&Path>,
    ) -> usize {
        let mut refused = 0;
        let mut queue = self.lock();
        for link in links {
            if admitted(link, shared) {
                queue.push(link.to_string());
            } else {
                refused += 1;
            }
        }
        refused
    }

    /// Every queued link, in arrival order, emptying the queue.
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
