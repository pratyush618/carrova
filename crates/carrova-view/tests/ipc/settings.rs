//! Settings JSON: files saved by older versions keep loading.

use carrova_view::ipc::{AlertChannel, Settings};
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
