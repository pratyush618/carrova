//! [`DownloadSink`]: a file being saved into the user's Downloads by the OS
//! ([`crate::Platform::downloads_sink`]).

use std::fmt;
use std::io;
use std::path::PathBuf;

use crate::PlatformError;

/// A new file in the user's Downloads, hidden from other apps until
/// [`DownloadSink::commit`]. Dropping it without committing removes it.
pub trait DownloadSink: Send + fmt::Debug {
    /// Write `bytes` at `offset`.
    ///
    /// # Errors
    ///
    /// The OS's write error.
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()>;

    /// Publish the file; returns where it is (its name may differ from the
    /// one asked for: the OS does not overwrite an existing file).
    ///
    /// # Errors
    ///
    /// [`PlatformError`] if the file cannot be flushed or published.
    fn commit(self: Box<Self>) -> Result<PathBuf, PlatformError>;

    /// Remove the unfinished file.
    fn abort(self: Box<Self>);
}
