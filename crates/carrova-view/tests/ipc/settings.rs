//! Settings JSON (files saved by older versions keep loading) and the save
//! policy that keeps saved wake hints in step.

use std::path::PathBuf;

use carrova_core::CoreError;
use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::KnownSecret;
use carrova_platform::fake::FakePlatform;
use carrova_platform::{OsKind, Platform};
use carrova_view::ipc::{AlertChannel, NearbySettings, Settings};
use serde_json::json;

#[test]
fn old_settings_file_loads_with_new_defaults() {
    let old = json!({
        "auto_check_recipes": true,
        "auto_check_app": false,
        "theme": null,
        "reduce_motion": null,
    });
    let settings: Settings = serde_json::from_value(old).unwrap();
    assert!(!settings.auto_check_app);
    assert!(settings.background.keep_running);
    assert!(!settings.background.start_at_login);
    assert!(settings.background.lean);
    let alerts = settings.alerts;
    assert!(alerts.system && alerts.wake_screen);
    assert!(!alerts.names_on_lock_screen);
    assert_eq!(alerts.paused_until, None);
    assert_eq!(alerts.requests, AlertChannel::PopupAndBell);
    assert_eq!(alerts.moves, AlertChannel::PopupAndBell);
    assert_eq!(alerts.nearby, AlertChannel::BellOnly);
    assert_eq!(alerts.updates, AlertChannel::BellOnly);
}

#[test]
fn old_settings_file_remembers_devices_and_sends_no_wake_hints() {
    let old = json!({
        "auto_check_recipes": true, "auto_check_app": true,
        "theme": null, "reduce_motion": null,
    });
    let settings: Settings = serde_json::from_value(old).unwrap();
    assert_eq!(
        settings.nearby,
        NearbySettings {
            remember: true,
            accept_known: true,
            wol_hints: false,
        }
    );
    let partial = json!({
        "auto_check_recipes": true, "auto_check_app": true,
        "theme": null, "reduce_motion": null,
        "nearby": { "remember": false },
    });
    let settings: Settings = serde_json::from_value(partial).unwrap();
    assert!(!settings.nearby.remember);
    assert!(settings.nearby.accept_known);
}

#[test]
fn missing_fields_inside_new_objects_take_their_defaults() {
    let partial = json!({
        "auto_check_recipes": true, "auto_check_app": true,
        "theme": null, "reduce_motion": null,
        "background": { "lean": false },
        "alerts": { "system": false, "paused_until": 99 },
    });
    let settings: Settings = serde_json::from_value(partial).unwrap();
    assert!(!settings.background.lean);
    assert!(settings.background.keep_running);
    assert!(!settings.alerts.system);
    assert!(settings.alerts.wake_screen);
    assert_eq!(settings.alerts.paused_until, Some(99));
    assert_eq!(settings.alerts.nearby, AlertChannel::BellOnly);
}

#[test]
fn unknown_keys_are_ignored() {
    let json = json!({
        "auto_check_recipes": true, "auto_check_app": true,
        "theme": null, "reduce_motion": null,
        "from_the_future": 1,
        "alerts": { "also_new": true },
    });
    assert!(serde_json::from_value::<Settings>(json).is_ok());
}

#[test]
fn alert_channel_round_trips_snake_case() {
    for (channel, text) in [
        (AlertChannel::PopupAndBell, "popup_and_bell"),
        (AlertChannel::BellOnly, "bell_only"),
    ] {
        let value = serde_json::to_value(channel).unwrap();
        assert_eq!(value, json!(text));
        assert_eq!(
            serde_json::from_value::<AlertChannel>(value).unwrap(),
            channel
        );
    }
}

/// A home with Carrova's state directory and the settings each write saw.
struct Home {
    _tmp: tempfile::TempDir,
    platform: FakePlatform,
    writes: Vec<bool>,
}

#[derive(Debug, PartialEq)]
enum Failed {
    Write,
    Hints,
}

impl From<CoreError> for Failed {
    fn from(_: CoreError) -> Self {
        Self::Hints
    }
}

impl Home {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let state = tmp.path().join("state");
        let platform = FakePlatform::with_home(tmp.path()).with_state_dir(state);
        Self {
            _tmp: tmp,
            platform,
            writes: Vec::new(),
        }
    }

    fn devices_dir(&self) -> PathBuf {
        self.platform.state_dir().unwrap().join("devices")
    }

    fn remember(&self, mac_hint: Option<[u8; 6]>) {
        let record = DeviceRecord {
            public: DeviceKey::from_bytes(&[1; 32]).public(),
            name: "Desk".into(),
            os: OsKind::Linux,
            first_seen: 100,
            last_move: None,
            moves: 0,
            mac_hint,
        };
        KnownDevices::open(&self.platform)
            .unwrap()
            .remember(&self.platform, record, &KnownSecret::from_bytes([1; 32]))
            .unwrap();
    }

    fn break_device_store(&self) {
        std::fs::write(self.devices_dir().join("known.json"), b"{not json").unwrap();
    }

    fn has_wake_hint(&self) -> bool {
        let devices = KnownDevices::open(&self.platform).unwrap();
        devices.list(&self.platform).unwrap()[0].mac_hint.is_some()
    }

    /// Save hints `old` → `new`, recording each write's `wol_hints`; every
    /// write fails if `fail_writes`.
    fn save(&mut self, old: bool, new: bool, fail_writes: bool) -> Result<(), Failed> {
        let writes = &mut self.writes;
        hints(new).save_over(&hints(old), &self.platform, |s| {
            writes.push(s.nearby.wol_hints);
            if fail_writes {
                Err(Failed::Write)
            } else {
                Ok(())
            }
        })
    }
}

fn hints(wol_hints: bool) -> Settings {
    let mut settings = Settings::default();
    settings.nearby.wol_hints = wol_hints;
    settings
}

const HINT: Option<[u8; 6]> = Some([2, 0, 0, 0, 0, 1]);

#[test]
fn hints_stay_while_wake_hints_stay_on() {
    let mut home = Home::new();
    home.remember(HINT);
    home.save(true, true, false).unwrap();
    assert!(home.has_wake_hint());
    assert_eq!(home.writes, [true]);
}

#[test]
fn turning_wake_hints_off_clears_the_saved_hints() {
    let mut home = Home::new();
    home.remember(HINT);
    home.save(true, false, false).unwrap();
    assert!(!home.has_wake_hint());
    assert_eq!(home.writes, [false]);
}

#[test]
fn hints_left_by_an_earlier_save_go_with_the_next_save_while_off() {
    let mut home = Home::new();
    home.remember(HINT);
    home.save(false, false, false).unwrap();
    assert!(!home.has_wake_hint());
}

#[test]
fn hints_that_cannot_be_dropped_leave_wake_hints_on() {
    let mut home = Home::new();
    home.remember(None);
    home.break_device_store();
    assert_eq!(home.save(true, false, false), Err(Failed::Hints));
    assert_eq!(home.writes, [false, true], "put back on");
}

#[test]
fn unreadable_hints_do_not_block_a_save_while_hints_are_off() {
    let mut home = Home::new();
    home.remember(None);
    home.break_device_store();
    home.save(false, false, false).unwrap();
    assert_eq!(home.writes, [false]);
}

#[test]
fn a_failed_save_turning_hints_off_still_drops_them() {
    let mut home = Home::new();
    home.remember(HINT);
    assert_eq!(home.save(true, false, true), Err(Failed::Write));
    assert!(!home.has_wake_hint(), "the file may say off already");
    assert_eq!(home.writes, [false], "no rollback after a failed save");
}

#[test]
fn a_failed_save_wins_over_hints_that_cannot_be_dropped() {
    let mut home = Home::new();
    home.remember(None);
    home.break_device_store();
    assert_eq!(home.save(true, false, true), Err(Failed::Write));
    assert_eq!(home.writes, [false]);
}

#[test]
fn a_save_with_hints_off_creates_no_device_store() {
    let mut home = Home::new();
    home.save(false, false, false).unwrap();
    assert!(
        !home.devices_dir().exists(),
        "nothing to clear on a fresh install"
    );
}
