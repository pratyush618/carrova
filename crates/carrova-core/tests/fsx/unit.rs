use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use carrova_platform::fake::{FakePlatform, Hooks};

use super::*;

/// A platform over `home` that counts lstat calls.
fn counting(home: &Path) -> (FakePlatform, Arc<AtomicUsize>) {
    let stats = Arc::new(AtomicUsize::new(0));
    let seen = stats.clone();
    let p = FakePlatform::with_home(home).with_hooks(Hooks {
        on_metadata: Some(Arc::new(move |_, _| {
            seen.fetch_add(1, Ordering::SeqCst);
        })),
        ..Hooks::default()
    });
    (p, stats)
}

#[test]
fn create_dir_or_existing_accepts_an_existing_directory() {
    let tmp = tempfile::tempdir().unwrap();
    let (p, _) = counting(tmp.path());
    let dir = tmp.path().join("d");
    create_dir_or_existing(&p, &dir, 0o700).unwrap();
    create_dir_or_existing(&p, &dir, 0o700).unwrap();
    assert!(dir.is_dir());
}

#[test]
fn create_dir_or_existing_refuses_a_file_in_the_way() {
    let tmp = tempfile::tempdir().unwrap();
    let (p, _) = counting(tmp.path());
    let file = tmp.path().join("f");
    std::fs::write(&file, b"x").unwrap();
    let err = create_dir_or_existing(&p, &file, 0o700).unwrap_err();
    assert_eq!(kind_of(&err), Some(io::ErrorKind::AlreadyExists));
}

#[test]
fn create_dir_or_existing_reports_other_failures_without_a_stat() {
    let tmp = tempfile::tempdir().unwrap();
    let (p, stats) = counting(tmp.path());
    // An existing directory, but the creation fails for another reason
    // (a mode outside 0o777): that error stands, nothing is stat'ed.
    let dir = tmp.path().join("d");
    std::fs::create_dir(&dir).unwrap();
    let err = create_dir_or_existing(&p, &dir, 0o4700).unwrap_err();
    assert_eq!(kind_of(&err), Some(io::ErrorKind::InvalidInput));
    assert_eq!(
        stats.load(Ordering::SeqCst),
        0,
        "stat after a non-EEXIST error"
    );
}
