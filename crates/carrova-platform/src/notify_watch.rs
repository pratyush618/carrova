//! Directory change notification on macOS (FSEvents) and Windows
//! (ReadDirectoryChangesW), through `notify`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::{DirWatch, PlatformError};

/// How often the watched dirs are checked for one removed or turned into a
/// file: neither backend reports that of a watched dir itself (Windows
/// drops the watch silently), nor one replaced while the watch restarted.
// shortcut: on Windows a watched dir deleted and re-created within one
// RECHECK, with no watched parent, keeps a dead watch (no change reported,
// no end); compare dir identity (file index) if a caller ever hits it.
const RECHECK: Duration = Duration::from_secs(2);

/// What wakes the watch thread.
enum Wake {
    Fs(notify::Result<Event>),
    Stop,
}

/// See [`crate::Platform::watch_dirs`]. One thread owns the watcher (whose
/// backend calls in from its own thread) and follows its events; the
/// guard's drop tells it to stop, then joins it.
pub(crate) fn watch_dirs(
    dirs: &[PathBuf],
    changed: Box<dyn Fn() + Send>,
) -> Result<DirWatch, PlatformError> {
    let dirs = dirs.to_vec();
    let (wake, wakes) = mpsc::channel();
    let (ready, started) = mpsc::channel();
    let events = wake.clone();
    let thread = thread::Builder::new()
        .name("carrova-watch".into())
        .spawn(move || match start(events, &dirs) {
            Ok((watcher, watched)) => {
                let _ = ready.send(Ok(()));
                follow(watcher, &wakes, &dirs, watched, &*changed);
            }
            Err(e) => {
                let _ = ready.send(Err(e));
            }
        })
        .map_err(|e| PlatformError::io("<watch thread>", e))?;
    let started = started
        .recv()
        .unwrap_or_else(|_| Err(io_err("<watch thread>", "ended before watching")));
    if let Err(e) = started {
        let _ = thread.join();
        return Err(e);
    }
    Ok(DirWatch::new(move || {
        let _ = wake.send(Wake::Stop);
        // A panicking callback has already ended the watch.
        let _ = thread.join();
    }))
}

/// A watcher sending its events to `events`, watching those of `dirs` that
/// exist now.
fn start(
    events: Sender<Wake>,
    dirs: &[PathBuf],
) -> Result<(RecommendedWatcher, Vec<PathBuf>), PlatformError> {
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = events.send(Wake::Fs(event));
    })
    .map_err(|e| io_err("<watch>", e))?;
    let watched = rewatch(&mut watcher, dirs, &[])?;
    Ok((watcher, watched))
}

/// Report changes until told to stop (or watching cannot go on).
fn follow(
    mut watcher: RecommendedWatcher,
    wakes: &mpsc::Receiver<Wake>,
    dirs: &[PathBuf],
    mut watched: Vec<PathBuf>,
    changed: &dyn Fn(),
) {
    loop {
        match wakes.recv_timeout(RECHECK) {
            Ok(Wake::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            Ok(Wake::Fs(Ok(event))) if !is_change(&event.kind) => continue,
            Err(RecvTimeoutError::Timeout)
                if present_dirs(dirs).is_ok_and(|now| now == watched) =>
            {
                continue;
            }
            // A backend error may mean a watch was lost: watch afresh.
            Ok(Wake::Fs(_)) | Err(RecvTimeoutError::Timeout) => {}
        }
        // Which entry changed does not matter: callers re-read what they
        // follow. Drain everything queued so a burst costs one call.
        if wakes.try_iter().any(|wake| matches!(wake, Wake::Stop)) {
            return;
        }
        let rewatched = rewatch(&mut watcher, dirs, &watched);
        changed();
        // A dir refused, or none left to report one appearing: end, so the
        // caller learns it as `changed` drops.
        match rewatched {
            Ok(now) if !now.is_empty() => watched = now,
            _ => return,
        }
    }
}

/// Whether `kind` creates, replaces, writes or removes an entry: opening
/// or locking one (access), or its metadata alone, is not a change.
fn is_change(kind: &EventKind) -> bool {
    !matches!(
        kind,
        EventKind::Access(_) | EventKind::Modify(ModifyKind::Metadata(_))
    )
}

/// Drop the watches of `watched` and watch each of `dirs` that exists now
/// afresh (one re-created since may hold a dead watch); returns those
/// watched.
fn rewatch(
    watcher: &mut RecommendedWatcher,
    dirs: &[PathBuf],
    watched: &[PathBuf],
) -> Result<Vec<PathBuf>, PlatformError> {
    let present = present_dirs(dirs)?;
    let mut paths = watcher.paths_mut();
    for dir in watched {
        // Fails where the watch went with its dir.
        let _ = paths.remove(dir);
    }
    let mut now = Vec::new();
    for dir in present {
        match paths.add(&dir, RecursiveMode::NonRecursive) {
            Ok(()) => now.push(dir),
            // Removed since it was found: watched once it appears again.
            Err(_) if !dir.exists() => {}
            Err(e) => return Err(io_err(&dir, e)),
        }
    }
    paths.commit().map_err(|e| io_err("<watch>", e))?;
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
