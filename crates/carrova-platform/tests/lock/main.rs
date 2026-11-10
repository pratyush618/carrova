//! `Platform::lock_exclusive`: an advisory, RAII exclusive file lock that
//! also serializes threads of one process.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::Duration;

use carrova_platform::{Platform, current};

#[test]
fn a_second_locker_waits_until_the_first_guard_is_dropped() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("lock");
    let platform: Arc<dyn Platform> = Arc::from(current());
    let held = Arc::new(AtomicBool::new(true));
    let guard = platform.lock_exclusive(&path).unwrap();

    let (tx, rx) = mpsc::channel();
    let waiter = {
        let (platform, path, held) = (Arc::clone(&platform), path.clone(), Arc::clone(&held));
        thread::spawn(move || {
            let _guard = platform.lock_exclusive(&path).unwrap();
            tx.send(held.load(Ordering::SeqCst)).unwrap();
        })
    };
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "second lock acquired while the first was held"
    );
    held.store(false, Ordering::SeqCst);
    drop(guard);
    let still_held = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(!still_held, "acquired before the first guard was released");
    waiter.join().unwrap();
}

#[test]
fn the_lock_file_is_created_and_reused() {
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("lock");
    let platform = current();
    drop(platform.lock_exclusive(&path).unwrap());
    assert!(path.is_file());
    drop(platform.lock_exclusive(&path).unwrap());
}

#[test]
#[cfg(unix)] // Windows: tests/windows/unit.rs (symlinks need privileges there)
fn a_symlink_is_never_locked_through() {
    let tmp = tempfile::tempdir().unwrap();
    let target = tmp.path().join("target");
    std::fs::write(&target, b"").unwrap();
    let link = tmp.path().join("lock");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert!(current().lock_exclusive(&link).is_err());
}
