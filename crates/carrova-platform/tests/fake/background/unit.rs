use crate::Platform;
use crate::fake::FakePlatform;
use std::path::Path;

#[test]
fn no_watcher_means_no_tray() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    assert!(p.has_status_tray());

    p.set_status_tray(false);
    assert!(!p.has_status_tray());
    // Clones share the knob, like the app's platform handle.
    assert!(!p.clone().has_status_tray());
}

#[test]
fn autostart_remembers_its_launcher() {
    let tmp = tempfile::TempDir::new().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    assert!(!p.autostart_status().unwrap());

    assert!(p.set_autostart(Path::new("/opt/carrova"), true).unwrap());
    assert_eq!(
        p.autostart_launcher().as_deref(),
        Some(Path::new("/opt/carrova"))
    );
    assert!(p.autostart_status().unwrap());

    assert!(!p.set_autostart(Path::new("/opt/carrova"), false).unwrap());
    assert_eq!(p.autostart_launcher(), None);
}
