//! Links the app was opened with: on a phone, the share sheet (files, as
//! `file:` links to copies, or shared text, queued as [`SHARED_TEXT`]
//! links), the quick-settings tile and
//! the notification's "Send clipboard" (`carrova://send-clipboard`).
//!
//! Any app may open Carrova with any link, so only [`admitted`] ones are
//! queued; the UI is told by [`OPENED_EVENT`], takes them with
//! `take_opened` and decides what to do with each. They are never logged
//! (shared text travels in them).

use std::path::{Component, Path};
use std::sync::{Mutex, PoisonError};

use data_encoding::BASE64;
use percent_encoding::{NON_ALPHANUMERIC, percent_decode_str, utf8_percent_encode};
use tauri::Url;

/// The Tauri event (no payload) telling the UI that links are queued.
pub const OPENED_EVENT: &str = "carrova://opened";

/// The link asking the UI to send the clipboard (tile, notification).
pub const SEND_CLIPBOARD: &str = "carrova://send-clipboard";

/// Where the share sheet's copies go, under the cache root (the Kotlin
/// `ShareActivity` writes `files/cache/shared`, the Android cache root).
pub const SHARED_DIR: &str = "shared";

/// How shared text reaches the UI: this prefix, then the text
/// percent-encoded as UTF-8 (whatever form the `data:` link had).
pub const SHARED_TEXT: &str = "data:text/plain,";

/// Whether `link` may reach the UI: [`SEND_CLIPBOARD`], shared text
/// (`http(s)`, or `data:text/plain` in UTF-8, see [`shared_text`]), or a
/// `file:` link to a share copy strictly inside `shared` (no `..`). Never
/// another file: the app could read its own private ones (its keys) and
/// send them away.
#[must_use]
pub fn admitted(link: &Url, shared: Option<&Path>) -> bool {
    match link.scheme() {
        "http" | "https" => true,
        "data" => shared_text(link).is_some(),
        "carrova" => link.as_str() == SEND_CLIPBOARD,
        "file" => shared.is_some_and(|shared| {
            link.to_file_path().is_ok_and(|path| {
                path.components().all(|c| {
                    matches!(
                        c,
                        Component::Prefix(_) | Component::RootDir | Component::Normal(_)
                    )
                }) && path.starts_with(shared)
                    && path != shared
            })
        }),
        _ => false,
    }
}

/// The text of a `data:text/plain` link: percent-encoded or `;base64`,
/// in UTF-8 (`;charset=utf-8` or `us-ascii`, or none). `None` for any
/// other data link (another type, charset or parameter, text that is not
/// UTF-8), so nothing is guessed.
#[must_use]
pub fn shared_text(link: &Url) -> Option<String> {
    // The whole link: text with `?` or `#` parses as a query or fragment.
    let (header, body) = link.as_str().strip_prefix("data:")?.split_once(',')?;
    let mut params = header.split(';');
    if !params.next()?.eq_ignore_ascii_case("text/plain") {
        return None;
    }
    let mut base64 = false;
    for param in params {
        match param.to_ascii_lowercase().as_str() {
            "base64" => base64 = true,
            "charset=utf-8" | "charset=us-ascii" => {}
            _ => return None,
        }
    }
    let bytes: Vec<u8> = percent_decode_str(body).collect();
    let bytes = if base64 {
        BASE64.decode(&bytes).ok()?
    } else {
        bytes
    };
    String::from_utf8(bytes).ok()
}

/// Admitted links waiting for the UI, in arrival order (one intent's
/// links arrive in no set order).
#[derive(Debug, Default)]
pub struct Opened(Mutex<Vec<String>>);

impl Opened {
    /// Queue the [`admitted`] `links` (share copies inside `shared`; shared
    /// text as a [`SHARED_TEXT`] link); returns how many were refused.
    pub fn push_admitted<'a>(
        &self,
        links: impl IntoIterator<Item = &'a Url>,
        shared: Option<&Path>,
    ) -> usize {
        let mut refused = 0;
        let mut queue = self.lock();
        for link in links {
            if !admitted(link, shared) {
                refused += 1;
            } else if let Some(text) = shared_text(link) {
                queue.push(format!(
                    "{SHARED_TEXT}{}",
                    utf8_percent_encode(&text, NON_ALPHANUMERIC)
                ));
            } else {
                queue.push(link.to_string());
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
