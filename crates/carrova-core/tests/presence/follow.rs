//! How a running [`Presence`] learns of a saved state changed elsewhere
//! (the CLI): by a change in the state dir where the platform can watch
//! it, else by checking every [`POLL`].

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use carrova_core::Engine;
use carrova_core::devices::DEVICES_DIR;
use carrova_core::presence::{POLL, Presence, VISIBILITY_FILE, Visibility, save_visibility};
use carrova_platform::Platform;
use carrova_platform::fake::Hooks;

use super::lan_support::Side;

/// A side whose platform counts reads of the saved visibility, watching
/// directories unless `unwatched`.
fn counting(unwatched: bool) -> (Side, Arc<AtomicUsize>) {
    let mut side = Side::new();
    let reads = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&reads);
    side.platform = Arc::new((*side.platform).clone().with_hooks(Hooks {
        on_open: Some(Arc::new(move |path: &Path| {
            if path.ends_with(VISIBILITY_FILE) {
                counter.fetch_add(1, Ordering::SeqCst);
            }
        })),
        unwatched,
        ..Hooks::default()
    }));
    side.engine = Engine::new(
        side.platform.clone(),
        side.engine.recipes().to_vec(),
        side.sink.clone(),
    );
    save_visibility(&*side.platform, Visibility::NoOne).unwrap();
    (side, reads)
}

/// Wait up to `within` for at least `n` reads.
fn reads_reach(reads: &AtomicUsize, n: usize, within: Duration) -> bool {
    let end = Instant::now() + within;
    while reads.load(Ordering::SeqCst) < n {
        if Instant::now() >= end {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    true
}

#[test]
fn a_change_in_the_state_dir_or_the_devices_dir_is_read_at_once() {
    let (side, reads) = counting(false);
    let presence = Presence::start(&side.engine);
    assert!(reads_reach(&reads, 1, POLL), "read on start");
    let state = side.platform.state_dir().unwrap();

    save_visibility(&*side.platform, Visibility::Known).unwrap();
    side.platform.fire_dir_change(&state);
    assert!(
        reads_reach(&reads, 2, Duration::from_secs(1)),
        "read on the change, well before a poll would"
    );
    side.platform.fire_dir_change(&state.join(DEVICES_DIR));
    assert!(reads_reach(&reads, 3, Duration::from_secs(1)));
    presence.stop();
}

#[test]
fn a_watched_presence_with_nothing_due_does_not_wake() {
    let (side, reads) = counting(false);
    let presence = Presence::start(&side.engine);
    assert!(reads_reach(&reads, 1, POLL), "read on start");
    std::thread::sleep(POLL + Duration::from_secs(1));
    assert_eq!(reads.load(Ordering::SeqCst), 1, "no timed re-read");
    presence.stop();
}

#[test]
fn without_a_watch_it_checks_every_poll() {
    let (side, reads) = counting(true);
    let presence = Presence::start(&side.engine);
    assert!(reads_reach(&reads, 1, POLL), "read on start");
    assert!(
        reads_reach(&reads, 2, POLL + Duration::from_secs(1)),
        "checked again after a poll"
    );
    presence.stop();
}
