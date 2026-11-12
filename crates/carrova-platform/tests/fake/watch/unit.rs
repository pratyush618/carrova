use std::sync::atomic::AtomicUsize;
use std::sync::mpsc;
use std::time::Duration;

use super::*;

#[test]
fn a_callback_may_change_the_watches() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    let p = FakePlatform::with_home(&dir);
    let inner = p.clone();
    let other = dir.join("other");
    let changed = Box::new(move || inner.end_dir_watches(&other));
    let watch = p.watch(std::slice::from_ref(&dir), changed).unwrap();
    let (done, finished) = mpsc::channel();
    // The guard goes with the firing thread: a deadlock fails the test
    // instead of hanging its unwind.
    std::thread::spawn(move || {
        p.fire_dir_change(&dir);
        drop(watch);
        done.send(()).unwrap();
    });
    finished
        .recv_timeout(Duration::from_secs(5))
        .expect("a callback changing the watches deadlocked");
}

#[test]
fn a_dropped_watch_is_never_called_back() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    let p = FakePlatform::with_home(&dir);
    let calls = Arc::new(AtomicUsize::new(0));
    let counted = Arc::clone(&calls);
    let changed = Box::new(move || {
        counted.fetch_add(1, Ordering::SeqCst);
    });
    let watch = p.watch(std::slice::from_ref(&dir), changed).unwrap();
    p.fire_dir_change(&dir);
    drop(watch);
    p.fire_dir_change(&dir);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[test]
fn an_ended_watch_drops_its_callback() {
    let tmp = tempfile::TempDir::new().unwrap();
    let dir = tmp.path().to_path_buf();
    let p = FakePlatform::with_home(&dir);
    let kept = Arc::new(());
    let held = Arc::clone(&kept);
    let _watch = p
        .watch(
            std::slice::from_ref(&dir),
            Box::new(move || {
                let _ = &held;
            }),
        )
        .unwrap();
    p.end_dir_watches(&dir);
    assert_eq!(Arc::strong_count(&kept), 1, "the callback was dropped");
}
