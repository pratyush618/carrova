//! Background mode: the Settings rows, the tray's pause item and the
//! autostart refresh at start-up.

use std::path::Path;
use std::sync::Arc;

use carrova_platform::Platform;
use carrova_platform::fake::FakePlatform;
use carrova_view::devices::VisibilityChoice;
use carrova_view::ipc::BackgroundSettings;
use carrova_view::tray::{CloseAction, PAUSE_SECS};
use serde_json::json;
use tauri::async_runtime::block_on;

use crate::ops::{background, devices, settings};
use crate::state::AppState;
use crate::testing::Machine;

/// All of `background` as a patch.
fn whole(background: BackgroundSettings) -> serde_json::Value {
    serde_json::to_value(background).unwrap()
}

/// A folder where the settings' temp file goes: nothing can be saved.
fn block_saves(machine: &Machine) {
    let blocker = machine
        .scratch("state")
        .join(format!("{}.tmp", crate::settings::SETTINGS_FILE));
    std::fs::create_dir_all(blocker.join("in-the-way")).unwrap();
}

/// `machine`'s state over a fake we keep a handle on.
fn with_fake(machine: &Machine) -> (AppState, FakePlatform) {
    let platform = FakePlatform::with_home(&machine.home).with_state_dir(machine.scratch("state"));
    let state = AppState {
        platform: Arc::new(platform.clone()),
        settings: crate::settings::SettingsStore::new(Arc::new(platform.clone())),
        ..machine.state.clone()
    };
    (state, platform)
}

#[test]
fn start_at_login_saves_and_writes_autostart() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    let mine = BackgroundSettings {
        start_at_login: true,
        ..BackgroundSettings::default()
    };

    block_on(background::set_background(&state, whole(mine))).unwrap();

    assert_eq!(block_on(settings::load(&state)).unwrap().background, mine);
    assert_eq!(
        platform.autostart_launcher(),
        Some(platform.launcher().unwrap())
    );
}

#[test]
fn keep_running_off_stops_autostart() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    platform
        .set_autostart(Path::new("/opt/carrova"), true)
        .unwrap();
    let mine = BackgroundSettings {
        keep_running: false,
        start_at_login: true,
        ..BackgroundSettings::default()
    };

    block_on(background::set_background(&state, whole(mine))).unwrap();

    assert_eq!(block_on(settings::load(&state)).unwrap().background, mine);
    assert_eq!(platform.autostart_launcher(), None);
}

#[test]
fn refresh_points_autostart_at_this_launcher() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    background::refresh_autostart(&state);
    assert_eq!(platform.autostart_launcher(), None, "off stays off");

    platform
        .set_autostart(Path::new("/old/carrova"), true)
        .unwrap();
    background::refresh_autostart(&state);
    assert_eq!(
        platform.autostart_launcher(),
        Some(platform.launcher().unwrap())
    );
}

#[test]
fn closing_follows_keep_running_and_the_tray() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    assert_eq!(background::close_action(&state), CloseAction::CloseToTray);

    // A session opening (or running a call) keeps its screen alive.
    let id = carrova_core::SessionId("20261112-101500-a1b2c3".into());
    state
        .sessions
        .opening(id.clone(), carrova_core::Cancel::default());
    assert_eq!(background::close_action(&state), CloseAction::HideToTray);
    state.sessions.abandon(&id);
    assert_eq!(background::close_action(&state), CloseAction::CloseToTray);

    platform.set_status_tray(false);
    assert_eq!(background::close_action(&state), CloseAction::Minimise);

    let off = BackgroundSettings {
        keep_running: false,
        ..BackgroundSettings::default()
    };
    block_on(background::set_background(&state, whole(off))).unwrap();
    assert_eq!(background::close_action(&state), CloseAction::Exit);
}

#[test]
fn the_tray_pause_item_pauses_then_resumes() {
    let machine = Machine::new();
    let (state, _) = with_fake(&machine);
    assert!(!background::tray_model(&state).paused);

    let model = background::toggle_pause(&state).unwrap();
    assert!(model.paused);
    let alerts = block_on(settings::load(&state)).unwrap().alerts;
    assert!(alerts.paused_until.is_some_and(|until| until >= PAUSE_SECS));

    assert!(!background::toggle_pause(&state).unwrap().paused);
    assert_eq!(
        block_on(settings::load(&state))
            .unwrap()
            .alerts
            .paused_until,
        None
    );
}

#[test]
fn about_says_whether_closing_can_go_to_the_tray() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    assert!(crate::ops::about::about(&state).status_tray);
    platform.set_status_tray(false);
    assert!(!crate::ops::about::about(&state).status_tray);
}

#[test]
fn the_tray_shows_a_running_move() {
    let machine = Machine::new();
    let (state, _) = with_fake(&machine);
    assert!(!background::tray_model(&state).moving);

    let id = carrova_core::SessionId("20261112-101500-a1b2c3".into());
    state
        .sessions
        .opening(id.clone(), carrova_core::Cancel::default());
    assert!(background::tray_model(&state).moving);
    state.sessions.abandon(&id);
    assert!(!background::tray_model(&state).moving);
}

#[test]
fn closing_keeps_the_window_when_lean_is_off() {
    let machine = Machine::new();
    let (state, _platform) = with_fake(&machine);
    let kept = BackgroundSettings {
        lean: false,
        ..BackgroundSettings::default()
    };
    block_on(background::set_background(&state, whole(kept))).unwrap();
    assert_eq!(background::close_action(&state), CloseAction::HideToTray);
}

#[test]
fn concurrent_setting_changes_all_survive() {
    let machine = Machine::new();
    let (state, _) = with_fake(&machine);
    let rounds = 100;
    let pauses = {
        let state = state.clone();
        std::thread::spawn(move || {
            for _ in 0..rounds {
                background::toggle_pause(&state).unwrap();
            }
        })
    };
    for round in 0..rounds {
        let lean = round % 2 == 1;
        block_on(background::set_background(&state, json!({ "lean": lean }))).unwrap();
    }
    pauses.join().unwrap();

    let saved = block_on(settings::load(&state)).unwrap();
    assert_eq!(saved.alerts.paused_until, None, "an even number of toggles");
    assert!(saved.background.lean, "the last change");
}

#[test]
fn refresh_waits_for_a_settings_change_in_progress() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    platform
        .set_autostart(Path::new("/old/carrova"), true)
        .unwrap();
    let writing = state.settings.lock();
    let refresh = {
        let state = state.clone();
        std::thread::spawn(move || background::refresh_autostart(&state))
    };
    std::thread::sleep(std::time::Duration::from_millis(100));
    assert_eq!(
        platform.autostart_launcher().as_deref(),
        Some(Path::new("/old/carrova")),
        "a change being saved decides first"
    );
    drop(writing);
    refresh.join().unwrap();
    assert_eq!(
        platform.autostart_launcher(),
        Some(platform.launcher().unwrap())
    );
}

#[test]
fn a_failed_save_leaves_autostart_as_it_was() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    block_saves(&machine);
    let mine = BackgroundSettings {
        start_at_login: true,
        ..BackgroundSettings::default()
    };

    assert!(block_on(background::set_background(&state, whole(mine))).is_err());

    assert_eq!(platform.autostart_launcher(), None);
    assert_eq!(
        block_on(settings::load(&state)).unwrap().background,
        BackgroundSettings::default()
    );
}

#[test]
fn a_settings_change_keeps_a_pause_set_in_the_tray() {
    let machine = Machine::new();
    let (state, _) = with_fake(&machine);
    background::toggle_pause(&state).unwrap();

    block_on(settings::save(&state, json!({ "theme": "dark" }))).unwrap();

    let saved = block_on(settings::load(&state)).unwrap();
    assert_eq!(saved.theme, Some(carrova_view::ipc::Theme::Dark));
    assert!(saved.alerts.paused_until.is_some());
}

#[test]
fn a_background_change_keeps_the_fields_it_does_not_name() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);

    block_on(background::set_background(
        &state,
        json!({ "keep_running": false }),
    ))
    .unwrap();
    block_on(background::set_background(
        &state,
        json!({ "start_at_login": true }),
    ))
    .unwrap();

    let saved = block_on(settings::load(&state)).unwrap().background;
    assert!(!saved.keep_running && saved.start_at_login);
    assert_eq!(
        platform.autostart_launcher(),
        None,
        "not while it does not keep running"
    );
}

#[test]
fn a_failed_save_restores_autostart_as_it_was_not_as_saved() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    let on = BackgroundSettings {
        start_at_login: true,
        ..BackgroundSettings::default()
    };
    block_on(background::set_background(&state, whole(on))).unwrap();
    // Turned off outside Carrova (the desktop's startup apps).
    platform
        .set_autostart(&platform.launcher().unwrap(), false)
        .unwrap();
    block_saves(&machine);

    assert!(block_on(background::set_background(&state, json!({ "lean": false }))).is_err());

    assert_eq!(platform.autostart_launcher(), None);
}

#[test]
fn a_change_that_keeps_autostart_leaves_its_entry_alone() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    let on = BackgroundSettings {
        start_at_login: true,
        ..BackgroundSettings::default()
    };
    block_on(background::set_background(&state, whole(on))).unwrap();
    // An entry Carrova did not just write (only start-up re-points it).
    platform
        .set_autostart(Path::new("/old/carrova"), true)
        .unwrap();

    block_on(background::set_background(&state, json!({ "lean": false }))).unwrap();

    assert_eq!(
        platform.autostart_launcher().as_deref(),
        Some(Path::new("/old/carrova"))
    );
}

#[test]
fn a_plain_settings_save_cannot_change_background() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);

    let patch = json!({ "background": { "start_at_login": true } });
    assert!(block_on(settings::save(&state, patch)).is_err());

    let saved = block_on(settings::load(&state)).unwrap();
    assert_eq!(saved.background, BackgroundSettings::default());
    assert_eq!(platform.autostart_launcher(), None);
}

#[test]
fn the_tray_shows_the_saved_visibility() {
    let machine = Machine::new();
    let (state, _) = with_fake(&machine);
    assert_eq!(
        background::tray_model(&state).visible_to,
        VisibilityChoice::Known
    );

    devices::apply_visibility(&state, VisibilityChoice::NoOne).unwrap();

    assert_eq!(
        background::tray_model(&state).visible_to,
        VisibilityChoice::NoOne
    );
}
