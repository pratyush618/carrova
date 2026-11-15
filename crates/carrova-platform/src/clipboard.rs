//! Clipboard types of [`crate::Platform::clipboard_read`] and friends.

use crate::watch::StopGuard;

/// What a [`ClipContent`] holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipKind {
    /// UTF-8 text.
    Text,
    /// A PNG image.
    Png,
}

/// The clipboard's content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipContent {
    /// How to read [`ClipContent::bytes`].
    pub kind: ClipKind,
    /// The content itself (never logged).
    pub bytes: Vec<u8>,
    /// The source marked it secret (a password manager's copy): callers
    /// must not sync it unless the user says so.
    pub sensitive: bool,
}

/// A clipboard being watched for changes, until the guard is dropped.
/// Dropping it waits for the watcher to stop, so the change callback is
/// never called afterwards.
#[derive(Debug)]
#[must_use = "the watch stops as soon as the guard is dropped"]
pub struct ClipboardWatch {
    _stop: StopGuard,
}

impl ClipboardWatch {
    /// Guard that runs `stop` (end the watcher, wait for it) on drop.
    /// (Android has no clipboard or directory watch.)
    #[cfg(any(feature = "fake", not(target_os = "android")))]
    pub(crate) fn new(stop: impl FnOnce() + Send + 'static) -> Self {
        Self {
            _stop: StopGuard::new(stop),
        }
    }
}
