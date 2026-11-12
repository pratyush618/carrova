//! Directory change notification on macOS (FSEvents) and Windows
//! (ReadDirectoryChangesW): the OS-neutral loop following a [`Backend`],
//! driven by `notify` in [`backend`].

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::PlatformError;

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod backend;
#[cfg(test)]
#[path = "../tests/notify_watch/unit.rs"]
mod tests;

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub(crate) use backend::watch_dirs;

/// Rewatches in a row that may fail before the watch ends: a dir can be
/// briefly unopenable (an antivirus scan, a sync client) without being gone.
const MAX_REWATCH_FAILURES: usize = 3;

/// A dir's identity, `(device or volume, inode or file index)`: a dir
/// re-created at the same path gets another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DirId(pub(crate) u64, pub(crate) u64);

/// A dir found present, with its identity where the OS gave one.
type Found = (PathBuf, Option<DirId>);

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

/// What a rewatch left watched.
struct Rewatched {
    watched: Vec<Found>,
    /// Why a dir present could not be watched; the next recheck retries it.
    failed: Option<PlatformError>,
}

/// Watch those of `dirs` that exist now; returns those watched.
fn first_watch(backend: &mut impl Backend, dirs: &[PathBuf]) -> Result<Vec<Found>, PlatformError> {
    let Rewatched { watched, failed } = rewatch(backend, dirs, &[])?;
    failed.map_or(Ok(watched), Err)
}

/// Report changes until told to stop (or watching cannot go on); the dirs
/// are rechecked `recheck` after the last recheck or change reported,
/// however much noise arrives meanwhile.
fn follow(
    backend: &mut impl Backend,
    wakes: &Receiver<Wake>,
    dirs: &[PathBuf],
    mut watched: Vec<Found>,
    changed: &dyn Fn(),
    recheck: Duration,
) {
    let mut next = Instant::now() + recheck;
    let mut failures = 0;
    loop {
        match wakes.recv_timeout(next.saturating_duration_since(Instant::now())) {
            Ok(Wake::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Wake::Noise) => continue,
            Err(RecvTimeoutError::Timeout)
                if present_dirs(dirs).is_ok_and(|now| unchanged(&now, &watched)) =>
            {
                // Every dir present is watched: no failure is pending.
                failures = 0;
                next = Instant::now() + recheck;
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
        next = Instant::now() + recheck;
        // A dir refused, one failing too often in a row, or none left to
        // report one appearing: end, so the caller learns it as `changed`
        // drops.
        let Ok(Rewatched {
            watched: now,
            failed,
        }) = rewatched
        else {
            return;
        };
        failures = if failed.is_some() { failures + 1 } else { 0 };
        if failures == MAX_REWATCH_FAILURES || (failed.is_none() && now.is_empty()) {
            return;
        }
        watched = now;
    }
}

/// Whether the dirs found `now` are those `watched`: the same paths, with
/// the same identities where both are known.
fn unchanged(now: &[Found], watched: &[Found]) -> bool {
    now.len() == watched.len()
        && now.iter().zip(watched).all(|((path, id), (was, was_id))| {
            path == was && id.zip(*was_id).is_none_or(|(id, was_id)| id == was_id)
        })
}

/// Drop the watches of `watched` and watch each of `dirs` that exists now
/// afresh (one re-created since may hold a dead watch); `Err` when one is
/// refused.
fn rewatch(
    backend: &mut impl Backend,
    dirs: &[PathBuf],
    watched: &[Found],
) -> Result<Rewatched, PlatformError> {
    let present = present_dirs(dirs)?;
    let old: Vec<_> = watched.iter().map(|(dir, _)| dir.clone()).collect();
    let new: Vec<_> = present.iter().map(|(dir, _)| dir.clone()).collect();
    let outcomes = match backend.rewatch(&old, &new) {
        Ok(outcomes) => outcomes,
        Err(e) => {
            return Ok(Rewatched {
                watched: Vec::new(),
                failed: Some(e),
            });
        }
    };
    let mut now = Rewatched {
        watched: Vec::new(),
        failed: None,
    };
    for (found, outcome) in present.into_iter().zip(outcomes) {
        match outcome {
            Ok(()) => now.watched.push(found),
            // Removed since it was found: watched once it appears again.
            Err(_) if !found.0.exists() => {}
            Err(e) => now.failed = Some(e),
        }
    }
    Ok(now)
}

/// Those of `dirs` that exist now, with their identities; one that is not
/// a directory (a symlink to one included) is refused.
fn present_dirs(dirs: &[PathBuf]) -> Result<Vec<Found>, PlatformError> {
    let mut present = Vec::new();
    for dir in dirs {
        match fs::symlink_metadata(dir) {
            Ok(meta) if meta.is_dir() => {
                present.push((dir.clone(), crate::sys::dir_id(dir, &meta).ok()));
            }
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
