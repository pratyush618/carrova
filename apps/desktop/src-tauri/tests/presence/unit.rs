#![allow(clippy::unwrap_used)]

use carrova_platform::fake::FakePlatform;
use tempfile::TempDir;

use super::*;

/// A machine named `name` (`None`: unnamed) in its own temp dir.
fn machine(tmp: &TempDir, name: Option<&str>) -> Arc<FakePlatform> {
    let dir = tmp.path().join(name.unwrap_or("stranger"));
    let platform = FakePlatform::with_home(dir.join("home")).with_state_dir(dir.join("state"));
    Arc::new(match name {
        Some(name) => platform.with_device_name(name),
        None => platform,
    })
}

#[test]
fn a_start_after_stop_does_nothing() {
    let tmp = TempDir::new().unwrap();
    let presence = BackgroundPresence::new(machine(&tmp, Some("desk")));
    presence.stop();
    presence.start();
    assert!(matches!(*presence.lock(), Running::Stopped));
}
