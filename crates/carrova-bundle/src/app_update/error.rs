//! What can go wrong with an app update. Messages never hold a URL (signed
//! download links carry tokens) or file contents.

use std::io;

use carrova_platform::PlatformError;

use super::{ApkRejected, FeedError};

/// A network failure, by kind only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum NetFailure {
    /// No connection to the server.
    #[error("the release server cannot be reached")]
    Connect,
    /// The server stopped answering.
    #[error("the connection timed out")]
    Timeout,
    /// The connection broke while reading.
    #[error("the connection broke ({0:?})")]
    Read(io::ErrorKind),
    /// The server refused (HTTP status).
    #[error("the release server answered {0}")]
    Status(u16),
    /// A redirect led off the hosts releases come from.
    #[error("the download left GitHub")]
    Untrusted,
    /// Too many redirects, or one without a location.
    #[error("the release server redirected badly")]
    Redirects,
    /// The release has no APK.
    #[error("this release has no Android download yet")]
    Missing,
    /// The body ended before its stated size.
    #[error("the download ended early")]
    Short,
}

/// An app update failed; nothing was installed.
#[derive(Debug, thiserror::Error)]
pub enum AppUpdateError {
    /// The user cancelled.
    #[error("the update was cancelled")]
    Cancelled,
    /// Another update runs now.
    #[error("an update is already running")]
    Busy,
    /// The release server or the connection failed.
    #[error("cannot download the update: {0}")]
    Network(#[from] NetFailure),
    /// The feed is not usable.
    #[error(transparent)]
    Feed(#[from] FeedError),
    /// The APK is larger than any release could be.
    #[error("the download is larger than an update can be")]
    TooLarge,
    /// The downloaded APK is not a trusted, newer Carrova (it was deleted).
    #[error(transparent)]
    Rejected(#[from] ApkRejected),
    /// The download could not be saved.
    #[error("cannot save the update")]
    Disk(#[source] io::Error),
    /// An OS call failed.
    #[error(transparent)]
    Platform(#[from] PlatformError),
}

impl From<io::Error> for AppUpdateError {
    fn from(error: io::Error) -> Self {
        Self::Disk(error)
    }
}
