//! Directory change notification (inotify).

use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::thread;

use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::inotify::{self, CreateFlags, WatchFlags};
use rustix::io::Errno;

use crate::{DirWatch, PlatformError};

/// Every way an atomic write (temp file + rename) or a removal shows up in
/// a directory; never through a symlinked dir.
const CHANGES: WatchFlags = WatchFlags::CREATE
    .union(WatchFlags::CLOSE_WRITE)
    .union(WatchFlags::MOVED_TO)
    .union(WatchFlags::MOVED_FROM)
    .union(WatchFlags::DELETE)
    .union(WatchFlags::ONLYDIR)
    .union(WatchFlags::DONT_FOLLOW);

/// See [`crate::Platform::watch_dirs`]. One thread polls the inotify fd
/// beside one end of a socket pair; the guard's drop closes the other end,
/// which wakes the thread to end, then joins it.
pub(super) fn watch_dirs(
    dirs: &[PathBuf],
    changed: Box<dyn Fn() + Send>,
) -> Result<DirWatch, PlatformError> {
    let fd = inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK)
        .map_err(|e| PlatformError::io("<inotify>", e.into()))?;
    let dirs = dirs.to_vec();
    add_watches(&fd, &dirs)?;
    let (stop, stopped) = UnixStream::pair().map_err(|e| PlatformError::io("<watch stop>", e))?;
    let thread = thread::Builder::new()
        .name("carrova-watch".into())
        .spawn(move || follow(&fd, &stopped, &dirs, &*changed))
        .map_err(|e| PlatformError::io("<watch thread>", e))?;
    Ok(DirWatch::new(move || {
        drop(stop);
        // A panicking callback has already ended the watch.
        let _ = thread.join();
    }))
}

/// Watch each of `dirs` that exists now (again for one re-created since:
/// it gets a fresh watch; an existing watch is kept).
fn add_watches(fd: &OwnedFd, dirs: &[PathBuf]) -> Result<(), PlatformError> {
    for dir in dirs {
        match inotify::add_watch(fd, dir.as_path(), CHANGES) {
            Ok(_) | Err(Errno::NOENT) => {}
            Err(e) => return Err(PlatformError::io(dir, e.into())),
        }
    }
    Ok(())
}

/// Report changes until `stopped` hangs up (or polling fails).
fn follow(fd: &OwnedFd, stopped: &UnixStream, dirs: &[PathBuf], changed: &dyn Fn()) {
    let mut events = [0u8; 4096];
    loop {
        let mut fds = [
            PollFd::new(fd, PollFlags::IN),
            PollFd::new(stopped, PollFlags::IN),
        ];
        match poll(&mut fds, None) {
            Ok(_) => {}
            Err(Errno::INTR) => continue,
            Err(_) => return,
        }
        if !fds[1].revents().is_empty() {
            return;
        }
        // Which entry changed does not matter: callers re-read what they
        // follow. Drain everything queued so a burst costs one call.
        while rustix::io::read(fd, &mut events).is_ok_and(|n| n > 0) {}
        // shortcut: a watch the inotify limit refuses after start is
        // skipped silently; report it if a caller ever needs to know.
        let _ = add_watches(fd, dirs);
        changed();
    }
}

#[cfg(test)]
#[path = "../../tests/linux/watch/unit.rs"]
mod tests;
