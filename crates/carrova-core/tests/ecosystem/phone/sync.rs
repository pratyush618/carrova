//! The messages sync window ("Keep in sync") store.

use carrova_core::ecosystem::SyncWindow;
use carrova_platform::fake::FakePlatform;
use carrova_platform::{PermissionModel, Platform};

fn platform(tmp: &tempfile::TempDir) -> FakePlatform {
    FakePlatform::with_home(tmp.path()).with_state_dir(tmp.path().join("state"))
}

#[test]
fn keep_in_sync_defaults_to_30_days() {
    let tmp = tempfile::tempdir().unwrap();
    assert_eq!(
        SyncWindow::open(&platform(&tmp)).unwrap(),
        SyncWindow::Days30
    );
}

#[test]
fn saved_window_survives_reopen_in_a_0600_versioned_file() {
    let tmp = tempfile::tempdir().unwrap();
    let p = platform(&tmp);
    SyncWindow::All.save(&p).unwrap();

    assert_eq!(SyncWindow::open(&p).unwrap(), SyncWindow::All);
    let path = tmp.path().join("state/phone/messages.json");
    if p.permission_model() == PermissionModel::Unix {
        let mode = p.metadata(&path).unwrap().mode.unwrap();
        assert_eq!(mode & 0o777, 0o600);
    }
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(json, serde_json::json!({ "version": 1, "keep": "all" }));
}

#[test]
fn newer_version_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let p = platform(&tmp);
    SyncWindow::Year.save(&p).unwrap();
    let path = tmp.path().join("state/phone/messages.json");
    std::fs::write(&path, r#"{ "version": 2, "keep": "all" }"#).unwrap();
    assert!(SyncWindow::open(&p).is_err());
}

#[test]
fn window_reaches_back_30_days_a_year_or_all() {
    let day = 24 * 60 * 60 * 1000;
    let now = 1000 * day;
    assert_eq!(SyncWindow::Days30.since_ms(now), Some(now - 30 * day));
    assert_eq!(SyncWindow::Year.since_ms(now), Some(now - 365 * day));
    assert_eq!(SyncWindow::All.since_ms(now), None);
}
