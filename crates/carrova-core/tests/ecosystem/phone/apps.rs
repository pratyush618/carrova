//! The notification-apps store.

use carrova_core::ecosystem::NotifyApps;
use carrova_platform::fake::FakePlatform;
use carrova_platform::{PermissionModel, Platform};

fn platform(tmp: &tempfile::TempDir) -> FakePlatform {
    FakePlatform::with_home(tmp.path()).with_state_dir(tmp.path().join("state"))
}

#[test]
fn new_app_defaults_on() {
    let tmp = tempfile::tempdir().unwrap();
    let apps = NotifyApps::open(&platform(&tmp)).unwrap();
    assert!(apps.allowed("org.example.chat"));
    assert!(apps.hide_codes() && apps.show_text());
}

#[test]
fn private_app_defaults_off() {
    let tmp = tempfile::tempdir().unwrap();
    let p = platform(&tmp);
    let mut apps = NotifyApps::open(&p).unwrap();
    assert!(!apps.allowed("com.snapwork.hdfc"));
    apps.set(&p, "com.snapwork.hdfc", true).unwrap();
    assert!(apps.allowed("com.snapwork.hdfc"));
    assert!(NotifyApps::open(&p).unwrap().allowed("com.snapwork.hdfc"));
}

#[test]
fn file_mode_0600_and_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let p = platform(&tmp);
    let mut apps = NotifyApps::open(&p).unwrap();
    apps.set(&p, "org.example.chat", false).unwrap();
    apps.set_show_text(&p, false).unwrap();

    let path = tmp.path().join("state/phone/notify-apps.json");
    if p.permission_model() == PermissionModel::Unix {
        let mode = p.metadata(&path).unwrap().mode.unwrap();
        assert_eq!(mode & 0o777, 0o600);
    }
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json["version"], 1);
    assert_eq!(json["apps"]["org.example.chat"], false);
    assert_eq!(json["hide_codes"], true);
    assert_eq!(json["show_text"], false);
    let reopened = NotifyApps::open(&p).unwrap();
    assert!(!reopened.allowed("org.example.chat") && !reopened.show_text());
}

#[test]
fn empty_app_id_is_not_private() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(NotifyApps::open(&platform(&tmp)).unwrap().allowed(""));
}
