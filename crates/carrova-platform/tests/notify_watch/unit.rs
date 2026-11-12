use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

use tempfile::TempDir;

/// Recheck period of the watches under test.
const RECHECK: Duration = Duration::from_millis(50);
/// Far longer than any wait a test expects to be met.
const SOON: Duration = Duration::from_secs(5);

/// A backend recording the dirs of each rewatch; while `failing` is above
/// zero, a rewatch fails every dir (and counts it down).
#[derive(Clone, Default)]
struct Fake {
    rewatched: Arc<Mutex<Vec<Vec<PathBuf>>>>,
    failing: Arc<AtomicUsize>,
}

impl Fake {
    fn failing(times: usize) -> Self {
        let fake = Self::default();
        fake.failing.store(times, Ordering::SeqCst);
        fake
    }

    fn rewatches(&self) -> usize {
        self.rewatched.lock().unwrap().len()
    }

    fn last(&self) -> Vec<PathBuf> {
        self.rewatched.lock().unwrap().last().cloned().unwrap()
    }
}

impl Backend for Fake {
    fn rewatch(
        &mut self,
        _old: &[PathBuf],
        dirs: &[PathBuf],
    ) -> Result<Vec<Result<(), PlatformError>>, PlatformError> {
        self.rewatched.lock().unwrap().push(dirs.to_vec());
        let fail = self
            .failing
            .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok();
        Ok(dirs
            .iter()
            .map(|dir| {
                if fail {
                    Err(io_err(dir, "busy"))
                } else {
                    Ok(())
                }
            })
            .collect())
    }
}

/// A watch of `dirs` following on its own thread.
struct Running {
    wake: Sender<Wake>,
    changes: mpsc::Receiver<()>,
    backend: Fake,
    thread: Option<JoinHandle<()>>,
}

fn run(dirs: &[PathBuf], backend: Fake) -> Running {
    let mut driven = backend.clone();
    let watched = first_watch(&mut driven, dirs).unwrap();
    let (wake, wakes) = mpsc::channel();
    let (tx, changes) = mpsc::channel();
    let dirs = dirs.to_vec();
    let thread = thread::spawn(move || {
        let changed = move || {
            let _ = tx.send(());
        };
        follow(&mut driven, &wakes, &dirs, watched, &changed, RECHECK);
    });
    Running {
        wake,
        changes,
        backend,
        thread: Some(thread),
    }
}

impl Running {
    /// Whether the watch reported a change within [`SOON`].
    fn changed(&self) -> bool {
        self.changes.recv_timeout(SOON).is_ok()
    }

    /// Whether the watch let go of its callback within [`SOON`] (changes
    /// it reported meanwhile are skipped).
    fn ends(&self) -> bool {
        let deadline = Instant::now() + SOON;
        loop {
            match self
                .changes
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            {
                Ok(()) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return true,
                Err(mpsc::RecvTimeoutError::Timeout) => return false,
            }
        }
    }

    /// Whether the watch is still following after a few rechecks.
    fn still_follows(&self) -> bool {
        let deadline = Instant::now() + RECHECK * 6;
        while let Some(left) = deadline.checked_duration_since(Instant::now()) {
            if let Err(mpsc::RecvTimeoutError::Disconnected) = self.changes.recv_timeout(left) {
                return false;
            }
        }
        true
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let _ = self.wake.send(Wake::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn dirs(tmp: &TempDir, names: &[&str]) -> Vec<PathBuf> {
    names
        .iter()
        .map(|name| {
            let dir = tmp.path().join(name);
            fs::create_dir(&dir).unwrap();
            dir
        })
        .collect()
}

#[test]
fn a_missing_dir_drops_out_and_the_rest_stay_watched() {
    let tmp = TempDir::new().unwrap();
    let dirs = dirs(&tmp, &["a", "b"]);
    let watch = run(&dirs, Fake::default());
    fs::remove_dir(&dirs[1]).unwrap();
    assert!(watch.changed(), "the recheck missed a removed dir");
    assert_eq!(watch.backend.last(), [dirs[0].clone()]);
    assert!(watch.still_follows());
}

#[test]
fn a_dir_turned_into_a_file_ends_the_watch() {
    let tmp = TempDir::new().unwrap();
    let dirs = dirs(&tmp, &["a", "b"]);
    let watch = run(&dirs, Fake::default());
    fs::remove_dir(&dirs[1]).unwrap();
    fs::write(&dirs[1], b"").unwrap();
    assert!(watch.ends());
}

#[test]
fn noise_is_not_a_change() {
    let tmp = TempDir::new().unwrap();
    let watch = run(&dirs(&tmp, &["a"]), Fake::default());
    watch.wake.send(Wake::Noise).unwrap();
    assert_eq!(
        watch.changes.recv_timeout(RECHECK * 4),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
}

#[test]
fn a_failure_on_the_first_watch_is_returned() {
    let tmp = TempDir::new().unwrap();
    let dirs = dirs(&tmp, &["a"]);
    assert!(first_watch(&mut Fake::failing(1), &dirs).is_err());
}

#[test]
fn a_change_is_reported_and_the_dirs_watched_afresh() {
    let tmp = TempDir::new().unwrap();
    let watch = run(&dirs(&tmp, &["a"]), Fake::default());
    watch.wake.send(Wake::Changed).unwrap();
    assert!(watch.changed());
    assert_eq!(watch.backend.rewatches(), 2, "first watch, then afresh");
}

#[test]
fn a_dir_replaced_between_rechecks_is_watched_afresh() {
    let tmp = TempDir::new().unwrap();
    let dirs = dirs(&tmp, &["a"]);
    let watch = run(&dirs, Fake::default());
    // The old dir outlives the new one's creation, so its id is not reused.
    let old = tmp.path().join("a.old");
    fs::rename(&dirs[0], &old).unwrap();
    fs::create_dir(&dirs[0]).unwrap();
    fs::remove_dir(&old).unwrap();
    assert!(watch.changed(), "the recheck missed a replaced dir");
    assert_eq!(watch.backend.last(), dirs);
}
