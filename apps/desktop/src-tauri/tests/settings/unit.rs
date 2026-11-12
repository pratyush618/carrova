#![allow(clippy::unwrap_used)]

use carrova_platform::fake::FakePlatform;
use carrova_view::ipc::Theme;

use super::*;

fn store(dir: &std::path::Path) -> SettingsStore {
    SettingsStore::new(Arc::new(
        FakePlatform::with_home(dir.join("home")).with_state_dir(dir.join("state")),
    ))
}

#[test]
fn defaults_until_something_is_saved() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(store(dir.path()).load().unwrap(), Settings::default());
}

#[test]
fn saved_settings_round_trip_and_replace_the_old_ones() {
    let dir = tempfile::tempdir().unwrap();
    let store = store(dir.path());
    let mut settings = Settings {
        theme: Some(Theme::Dark),
        ..Settings::default()
    };
    store.save(&settings).unwrap();
    assert_eq!(store.load().unwrap(), settings);
    settings.auto_check_app = false;
    store.save(&settings).unwrap();
    assert_eq!(store.load().unwrap(), settings);
    let leftovers: Vec<_> = std::fs::read_dir(dir.path().join("state"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(leftovers, [SETTINGS_FILE]);
}

#[test]
fn a_corrupt_file_is_an_error_not_silent_defaults() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("state")).unwrap();
    std::fs::write(dir.path().join("state").join(SETTINGS_FILE), "{").unwrap();
    assert!(store(dir.path()).load().is_err());
}

#[test]
fn a_patch_changes_only_the_fields_it_names() {
    let mut stored = Settings {
        theme: Some(Theme::Light),
        ..Settings::default()
    };
    stored.alerts.paused_until = Some(99);

    let patch = serde_json::json!({ "theme": null, "alerts": { "system": false } });
    let mut expected = Settings {
        theme: None,
        ..stored.clone()
    };
    expected.alerts.system = false;

    assert_eq!(patched(&stored, patch).unwrap(), expected);
}

#[test]
fn a_patch_that_breaks_the_settings_is_refused() {
    let wrong = serde_json::json!({ "alerts": { "requests": "loudly" } });
    assert!(patched(&Settings::default(), wrong).is_err());
}
