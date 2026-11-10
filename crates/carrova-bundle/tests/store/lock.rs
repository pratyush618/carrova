//! Store changes from concurrent threads (and processes) serialize on the
//! store lock.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use carrova_bundle::{BundleVersion, DirFetcher, Updater};
use carrova_platform::fake::Hooks;

use crate::support::{Keys, NOW, bundle, platform, store};

#[test]
fn concurrent_updates_in_one_process_serialize() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (v1, v2) = (tmp.path().join("v1"), tmp.path().join("v2"));
    bundle(&keys, tmp.path(), &v1, 1, "Alpha");
    bundle(&keys, tmp.path(), &v2, 2, "Alpha two");

    // The first install to reach its index rename parks there until
    // released; no other install may reach its own rename meanwhile.
    let renames = Arc::new(AtomicUsize::new(0));
    let (parked_tx, parked_rx) = mpsc::channel::<()>();
    let (release_tx, release_rx) = mpsc::channel::<()>();
    let gate = Arc::new(Mutex::new(Some((parked_tx, release_rx))));
    let hooks = Hooks {
        on_rename: Some(Arc::new({
            let (renames, gate) = (Arc::clone(&renames), Arc::clone(&gate));
            move |_, _| {
                renames.fetch_add(1, Ordering::SeqCst);
                if let Some((parked, release)) = gate.lock().unwrap().take() {
                    parked.send(()).unwrap();
                    release.recv().unwrap();
                }
                Ok(())
            }
        })),
        ..Hooks::default()
    };
    let p = platform(&tmp.path().join("home")).with_hooks(hooks);

    let update = &|dir: &std::path::Path| {
        Updater::new(DirFetcher::new(dir), keys.roots.clone()).update(&store(&p, &keys.roots), NOW)
    };
    thread::scope(|s| {
        let first = s.spawn(move || update(&v1));
        parked_rx.recv().unwrap();
        let second = s.spawn(move || update(&v2));
        thread::sleep(Duration::from_millis(300));
        assert_eq!(
            renames.load(Ordering::SeqCst),
            1,
            "second update ran while the first held the store"
        );
        release_tx.send(()).unwrap();
        first.join().unwrap().unwrap();
        second.join().unwrap().unwrap();
    });
    assert_eq!(renames.load(Ordering::SeqCst), 2);
    let installed = store(&p, &keys.roots).installed().unwrap().unwrap();
    assert_eq!(installed.version(), BundleVersion(2));
    assert_eq!(installed.previous, Some(BundleVersion(1)));
}
