//! Background mode: the Settings rows, the tray's pause item and the
//! autostart refresh at start-up.

use std::path::Path;
use std::sync::Arc;

use carrova_platform::Platform;
use carrova_platform::fake::FakePlatform;
use carrova_view::ipc::BackgroundSettings;
use carrova_view::tray::{CloseAction, PAUSE_SECS};
use tauri::async_runtime::block_on;

use crate::ops::{background, settings};
use crate::state::AppState;
use crate::testing::Machine;

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

    block_on(background::set_background(&state, mine)).unwrap();

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

    block_on(background::set_background(&state, mine)).unwrap();

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
    block_on(background::set_background(&state, off)).unwrap();
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
    block_on(background::set_background(&state, kept)).unwrap();
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
        block_on(background::set_background(
            &state,
            BackgroundSettings {
                lean,
                ..BackgroundSettings::default()
            },
        ))
        .unwrap();
    }
    pauses.join().unwrap();

    let saved = block_on(settings::load(&state)).unwrap();
    assert_eq!(saved.alerts.paused_until, None, "an even number of toggles");
    assert!(saved.background.lean, "the last change");
}
