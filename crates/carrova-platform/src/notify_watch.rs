//! Directory change notification on macOS (FSEvents) and Windows
//! (ReadDirectoryChangesW): the OS-neutral loop following a [`Backend`],
//! driven by `notify` in [`backend`].

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Duration;

use crate::PlatformError;

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod backend;
#[cfg(test)]
#[path = "../tests/notify_watch/unit.rs"]
mod tests;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) use backend::watch_dirs;

/// What wakes the watch thread.
enum Wake {
    /// An entry changed, or a watch may have been lost.
    Changed,
    /// An event that changes no entry (access, metadata).
    Noise,
    /// The guard dropped.
    Stop,
}

/// The OS watch [`follow`] drives.
trait Backend {
    /// Drop the watches of `old` (one gone with its dir may fail to drop),
    /// then watch each of `dirs`: the outcome of each, in order, or `Err`
    /// when applying them failed as a whole.
    fn rewatch(
        &mut self,
        old: &[PathBuf],
        dirs: &[PathBuf],
    ) -> Result<Vec<Result<(), PlatformError>>, PlatformError>;
}

/// Watch those of `dirs` that exist now; returns those watched.
fn first_watch(
    backend: &mut impl Backend,
    dirs: &[PathBuf],
) -> Result<Vec<PathBuf>, PlatformError> {
    rewatch(backend, dirs, &[])
}

/// Report changes until told to stop (or watching cannot go on); the dirs
/// are rechecked after `recheck` without a change.
fn follow(
    backend: &mut impl Backend,
    wakes: &Receiver<Wake>,
    dirs: &[PathBuf],
    mut watched: Vec<PathBuf>,
    changed: &dyn Fn(),
    recheck: Duration,
) {
    loop {
        match wakes.recv_timeout(recheck) {
            Ok(Wake::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Wake::Noise) => continue,
            Err(RecvTimeoutError::Timeout)
                if present_dirs(dirs).is_ok_and(|now| now == watched) =>
            {
                continue;
            }
            Ok(Wake::Changed) | Err(RecvTimeoutError::Timeout) => {}
        }
        // Which entry changed does not matter: callers re-read what they
        // follow. Drain everything queued so a burst costs one call.
        if wakes.try_iter().any(|wake| matches!(wake, Wake::Stop)) {
            return;
        }
        let rewatched = rewatch(backend, dirs, &watched);
        changed();
        // A dir refused, or none left to report one appearing: end, so the
        // caller learns it as `changed` drops.
        match rewatched {
            Ok(now) if !now.is_empty() => watched = now,
            _ => return,
        }
    }
}

/// Drop the watches of `watched` and watch each of `dirs` that exists now
/// afresh (one re-created since may hold a dead watch); returns those
/// watched.
fn rewatch(
    backend: &mut impl Backend,
    dirs: &[PathBuf],
    watched: &[PathBuf],
) -> Result<Vec<PathBuf>, PlatformError> {
    let present = present_dirs(dirs)?;
    let outcomes = backend.rewatch(watched, &present)?;
    let mut now = Vec::new();
    for (dir, outcome) in present.into_iter().zip(outcomes) {
        match outcome {
            Ok(()) => now.push(dir),
            // Removed since it was found: watched once it appears again.
            Err(_) if !dir.exists() => {}
            Err(e) => return Err(e),
        }
    }
    Ok(now)
}

/// Those of `dirs` that exist now; one that is not a directory (a symlink
/// to one included) is refused.
fn present_dirs(dirs: &[PathBuf]) -> Result<Vec<PathBuf>, PlatformError> {
    let mut present = Vec::new();
    for dir in dirs {
        match fs::symlink_metadata(dir) {
            Ok(meta) if meta.is_dir() => present.push(dir.clone()),
            Ok(_) => return Err(io_err(dir, "not a directory")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(PlatformError::io(dir, e)),
        }
    }
    Ok(present)
}

fn io_err(
    path: impl AsRef<Path>,
    error: impl Into<Box<dyn std::error::Error + Send + Sync>>,
) -> PlatformError {
    PlatformError::io(path.as_ref(), io::Error::other(error))
}
