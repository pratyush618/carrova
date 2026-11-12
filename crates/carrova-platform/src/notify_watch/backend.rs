//! The `notify` side of the watch: FSEvents on macOS,
//! ReadDirectoryChangesW on Windows.

use std::path::PathBuf;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::Duration;

use notify::event::ModifyKind;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use super::{Backend, Found, Wake, first_watch, follow, io_err};
use crate::{DirWatch, PlatformError};

/// How often the watched dirs are checked for one removed, re-created or
/// turned into a file: neither backend reports that of a watched dir
/// itself (Windows drops the watch silently), nor one replaced while the
/// watch restarted.
const RECHECK: Duration = Duration::from_secs(2);

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
            Ok((mut watcher, watched)) => {
                let _ = ready.send(Ok(()));
                follow(&mut watcher, &wakes, &dirs, watched, &*changed, RECHECK);
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
) -> Result<(RecommendedWatcher, Vec<Found>), PlatformError> {
    let mut watcher = notify::recommended_watcher(move |event| {
        let _ = events.send(wake_for(event));
    })
    .map_err(|e| io_err("<watch>", e))?;
    let watched = first_watch(&mut watcher, dirs)?;
    Ok((watcher, watched))
}

/// What `event` asks of the watch: a backend error may mean a watch was
/// lost, so it counts as a change (and the dirs are watched afresh).
fn wake_for(event: notify::Result<Event>) -> Wake {
    match event {
        Ok(event) if !is_change(&event.kind) => Wake::Noise,
        _ => Wake::Changed,
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

impl Backend for RecommendedWatcher {
    fn rewatch(
        &mut self,
        old: &[PathBuf],
        dirs: &[PathBuf],
    ) -> Result<Vec<Result<(), PlatformError>>, PlatformError> {
        let mut paths = self.paths_mut();
        for dir in old {
            // Fails where the watch went with its dir.
            let _ = paths.remove(dir);
        }
        let added = dirs
            .iter()
            .map(|dir| {
                paths
                    .add(dir, RecursiveMode::NonRecursive)
                    .map_err(|e| io_err(dir, e))
            })
            .collect();
        paths.commit().map_err(|e| io_err("<watch>", e))?;
        Ok(added)
    }
}
