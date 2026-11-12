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

/// A backend recording the `(old, dirs)` of each rewatch; while `failing`
/// is above zero, a rewatch fails every dir, and while `batch_failing` is,
/// the rewatch as a whole (each counts itself down).
#[derive(Clone, Default)]
struct Fake {
    rewatched: Arc<Mutex<Vec<Call>>>,
    failing: Arc<AtomicUsize>,
    batch_failing: Arc<AtomicUsize>,
}

/// The `(old, dirs)` of one rewatch.
type Call = (Vec<PathBuf>, Vec<PathBuf>);

/// Count `n` down; whether it was above zero.
fn take(n: &AtomicUsize) -> bool {
    n.try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
        .is_ok()
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

    fn last(&self) -> Call {
        self.rewatched.lock().unwrap().last().cloned().unwrap()
    }
}

impl Backend for Fake {
    fn rewatch(
        &mut self,
        old: &[PathBuf],
        dirs: &[PathBuf],
    ) -> Result<Vec<Result<(), PlatformError>>, PlatformError> {
        self.rewatched
            .lock()
            .unwrap()
            .push((old.to_vec(), dirs.to_vec()));
        if take(&self.batch_failing) {
            return Err(io_err("<watch>", "stream did not start"));
        }
        let fail = take(&self.failing);
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
    assert_eq!(watch.backend.last().1, [dirs[0].clone()]);
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
    assert_eq!(watch.backend.last().1, dirs);
}

#[test]
fn an_unknown_watched_identity_does_not_match_a_known_one() {
    let at = |id: Option<(u64, u64)>| vec![(PathBuf::from("a"), id.map(|(d, i)| DirId(d, i)))];
    let cases = [
        (None, Some((1, 1)), false),
        (Some((1, 1)), Some((1, 1)), true),
        (Some((1, 1)), Some((1, 2)), false),
        (Some((1, 1)), None, true),
        (None, None, true),
    ];
    for (watched, now, same) in cases {
        assert_eq!(
            unchanged(&at(now), &at(watched)),
            same,
            "{watched:?} -> {now:?}"
        );
    }
}

#[test]
fn noise_does_not_put_off_the_recheck() {
    let tmp = TempDir::new().unwrap();
    let dirs = dirs(&tmp, &["a", "b"]);
    let watch = run(&dirs, Fake::default());
    fs::remove_dir(&dirs[1]).unwrap();
    let noise = watch.wake.clone();
    let stream = thread::spawn(move || {
        // Until the watch ends.
        while noise.send(Wake::Noise).is_ok() {
            thread::sleep(RECHECK / 5);
        }
    });
    assert!(watch.changed(), "a stream of noise put off the recheck");
    drop(watch);
    stream.join().unwrap();
}

#[test]
fn noise_queued_without_a_pause_does_not_put_off_the_recheck() {
    let tmp = TempDir::new().unwrap();
    let dirs = dirs(&tmp, &["a", "b"]);
    let watch = run(&dirs, Fake::default());
    fs::remove_dir(&dirs[1]).unwrap();
    let noise = watch.wake.clone();
    let stream = thread::spawn(move || {
        // Bursts keep the queue from ever running dry, for a bounded time.
        let end = Instant::now() + RECHECK * 20;
        while Instant::now() < end {
            for _ in 0..256 {
                let _ = noise.send(Wake::Noise);
            }
            thread::yield_now();
        }
    });
    let seen = watch.changes.recv_timeout(RECHECK * 10).is_ok();
    stream.join().unwrap();
    assert!(seen, "queued noise put off the recheck");
}

#[test]
fn a_rewatch_failing_short_of_the_limit_is_retried() {
    let tmp = TempDir::new().unwrap();
    let dirs = dirs(&tmp, &["a"]);
    let backend = Fake::default();
    let watch = run(&dirs, backend.clone());
    backend
        .failing
        .store(MAX_REWATCH_FAILURES - 1, Ordering::SeqCst);
    watch.wake.send(Wake::Changed).unwrap();
    // The first watch, the failures, then one that took.
    let settled = 1 + (MAX_REWATCH_FAILURES - 1) + 1;
    let deadline = Instant::now() + SOON;
    while backend.rewatches() < settled && Instant::now() < deadline {
        thread::sleep(RECHECK / 5);
    }
    assert!(watch.still_follows(), "a passing failure ended the watch");
    assert_eq!(backend.rewatches(), settled);
    assert!(watch.still_follows());
    assert_eq!(backend.rewatches(), settled, "settled");
}

#[test]
fn a_rewatch_failing_up_to_the_limit_ends_the_watch() {
    let tmp = TempDir::new().unwrap();
    let backend = Fake::default();
    let watch = run(&dirs(&tmp, &["a"]), backend.clone());
    backend.failing.store(usize::MAX, Ordering::SeqCst);
    watch.wake.send(Wake::Changed).unwrap();
    assert!(watch.ends());
    // The first watch, the one the change asked for, then a failing recheck
    // per allowed failure.
    assert_eq!(backend.rewatches(), 2 + MAX_REWATCH_FAILURES);
}

#[test]
fn rewatches_failing_on_changes_count_once_per_recheck() {
    let tmp = TempDir::new().unwrap();
    let backend = Fake::default();
    let watch = run(&dirs(&tmp, &["a"]), backend.clone());
    backend
        .failing
        .store(MAX_REWATCH_FAILURES, Ordering::SeqCst);
    for _ in 0..MAX_REWATCH_FAILURES {
        watch.wake.send(Wake::Changed).unwrap();
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        watch.still_follows(),
        "a burst of changes used up the failures allowed"
    );
}

#[test]
fn a_rewatch_after_a_failed_batch_drops_the_watches_it_tried() {
    let tmp = TempDir::new().unwrap();
    let dirs = dirs(&tmp, &["a"]);
    let backend = Fake::default();
    let watch = run(&dirs, backend.clone());
    backend.batch_failing.store(1, Ordering::SeqCst);
    watch.wake.send(Wake::Changed).unwrap();
    assert!(watch.changed());
    // The recheck finds `a` unwatched and tries again.
    assert!(watch.changed());
    let (old, added) = backend.last();
    assert_eq!(added, dirs);
    assert!(
        old.contains(&dirs[0]),
        "a half-applied watch may stay: {old:?}"
    );
}

#[test]
fn a_dir_whose_metadata_fails_is_still_watched_and_retried() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("file");
    fs::write(&file, b"").unwrap();
    // Under a regular file: ENOTDIR on Linux, not NotFound.
    let dir = file.join("child");
    // Windows and others may say NotFound (a missing dir, dropped): skip there.
    if fs::symlink_metadata(&dir).is_err_and(|e| e.kind() == io::ErrorKind::NotFound) {
        return;
    }
    let backend = Fake::default();
    let watch = run(&[dir], backend.clone());
    backend
        .failing
        .store(MAX_REWATCH_FAILURES - 1, Ordering::SeqCst);
    watch.wake.send(Wake::Changed).unwrap();
    // The first watch, the failures, then one that took.
    let settled = 1 + (MAX_REWATCH_FAILURES - 1) + 1;
    let deadline = Instant::now() + SOON;
    while backend.rewatches() < settled && Instant::now() < deadline {
        thread::sleep(RECHECK / 5);
    }
    assert!(watch.still_follows(), "a metadata error ended the watch");
    assert_eq!(backend.rewatches(), settled, "the failures were retried");
}
