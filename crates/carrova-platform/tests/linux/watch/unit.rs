use super::*;
use std::fs;
use std::path::Path;
use std::sync::mpsc;
use std::time::Duration;
use tempfile::TempDir;

const SOON: Duration = Duration::from_secs(5);

/// Watch `dirs`, each change reported on the returned receiver.
fn watching(dirs: &[PathBuf]) -> (DirWatch, mpsc::Receiver<()>) {
    let (tx, rx) = mpsc::channel();
    let watch = watch_dirs(
        dirs,
        Box::new(move || {
            let _ = tx.send(());
        }),
    )
    .unwrap();
    (watch, rx)
}

/// Write `dir/name` the way Carrova saves a document: temp file + rename.
fn replace_atomically(dir: &Path, name: &str) {
    let temp = dir.join(format!(".{name}.tmp"));
    fs::write(&temp, b"new").unwrap();
    fs::rename(&temp, dir.join(name)).unwrap();
}

#[test]
fn an_atomic_replace_in_a_watched_dir_is_reported() {
    let tmp = TempDir::new().unwrap();
    fs::write(tmp.path().join("presence.json"), b"old").unwrap();
    let (_watch, changes) = watching(&[tmp.path().to_path_buf()]);
    replace_atomically(tmp.path(), "presence.json");
    changes.recv_timeout(SOON).unwrap();
}

#[test]
fn a_watched_dir_that_appears_later_is_watched_from_then_on() {
    let tmp = TempDir::new().unwrap();
    let devices = tmp.path().join("devices");
    let (_watch, changes) = watching(&[tmp.path().to_path_buf(), devices.clone()]);
    fs::create_dir(&devices).unwrap();
    changes.recv_timeout(SOON).unwrap();
    while changes.try_recv().is_ok() {}
    replace_atomically(&devices, "known.json");
    changes.recv_timeout(SOON).unwrap();
}

#[test]
fn dropping_the_watch_stops_it() {
    let tmp = TempDir::new().unwrap();
    let (watch, changes) = watching(&[tmp.path().to_path_buf()]);
    drop(watch);
    assert_eq!(
        changes.recv_timeout(SOON),
        Err(mpsc::RecvTimeoutError::Disconnected),
        "the watcher ended and let go of its callback"
    );
}

#[test]
fn a_dir_that_is_a_file_is_refused() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("file");
    fs::write(&file, b"").unwrap();
    assert!(watch_dirs(&[file], Box::new(|| {})).is_err());
}

/// Whether the watch let go of its callback within [`SOON`] (changes it
/// reported meanwhile are skipped).
fn ends(changes: &mpsc::Receiver<()>) -> bool {
    loop {
        match changes.recv_timeout(SOON) {
            Ok(()) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return true,
            Err(mpsc::RecvTimeoutError::Timeout) => return false,
        }
    }
}

#[test]
fn a_watched_dir_that_cannot_be_watched_again_ends_the_watch() {
    let tmp = TempDir::new().unwrap();
    let devices = tmp.path().join("devices");
    fs::create_dir(&devices).unwrap();
    let (_watch, changes) = watching(&[tmp.path().to_path_buf(), devices.clone()]);
    fs::remove_dir(&devices).unwrap();
    fs::write(&devices, b"").unwrap();
    assert!(ends(&changes), "a dir it misses must not go unreported");
}

#[test]
fn a_watch_left_with_no_dir_ends() {
    let tmp = TempDir::new().unwrap();
    let state = tmp.path().join("state");
    fs::create_dir(&state).unwrap();
    let (_watch, changes) = watching(std::slice::from_ref(&state));
    fs::remove_dir(&state).unwrap();
    assert!(ends(&changes), "nothing could wake it any more");
}

#[test]
fn taking_a_lock_in_a_watched_dir_is_not_a_change() {
    let tmp = TempDir::new().unwrap();
    let lock = tmp.path().join("known.lock");
    drop(crate::fsops::lock_exclusive(&lock).unwrap());
    let (_watch, changes) = watching(&[tmp.path().to_path_buf()]);
    drop(crate::fsops::lock_exclusive(&lock).unwrap());
    assert_eq!(
        changes.recv_timeout(Duration::from_millis(300)),
        Err(mpsc::RecvTimeoutError::Timeout),
        "a lock taken and released woke the watch"
    );
    // A secret written in place, like `write_new`, still is one.
    fs::write(tmp.path().join("peer.key"), b"secret").unwrap();
    changes.recv_timeout(SOON).unwrap();
}
