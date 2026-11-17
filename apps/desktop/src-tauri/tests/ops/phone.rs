//! The phone's own settings: its notification app list and how far back
//! its texts sync, kept in the files the giver reads.

use std::sync::Arc;

use carrova_core::ecosystem::{NotifyApps, SyncWindow};
use carrova_platform::fake::FakePlatform;
use tauri::async_runtime::block_on;

use crate::ops::phone;
use crate::state::AppState;
use crate::testing::Machine;

fn with_fake(machine: &Machine) -> (AppState, FakePlatform) {
    let platform = FakePlatform::with_home(&machine.home).with_state_dir(machine.scratch("state"));
    let state = AppState {
        platform: Arc::new(platform.clone()),
        ..machine.state.clone()
    };
    (state, platform)
}

#[test]
fn messages_sync_starts_at_30_days_and_keeps_the_choice() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);
    assert_eq!(
        block_on(phone::messages_sync(&state)).unwrap(),
        SyncWindow::Days30
    );

    block_on(phone::set_messages_sync(&state, SyncWindow::Year)).unwrap();

    assert_eq!(
        block_on(phone::messages_sync(&state)).unwrap(),
        SyncWindow::Year
    );
    assert_eq!(SyncWindow::open(&platform).unwrap(), SyncWindow::Year);
}

#[test]
fn notify_choices_reach_the_list_the_giver_reads() {
    let machine = Machine::new();
    let (state, platform) = with_fake(&machine);

    block_on(phone::set_notify_app(&state, "org.chat".into(), false)).unwrap();
    block_on(phone::set_notify_filters(&state, false, false)).unwrap();

    let view = block_on(phone::notify_apps(&state)).unwrap();
    assert_eq!(view.apps.len(), 1);
    assert!(!view.apps[0].on && !view.hide_codes && !view.show_text);
    let saved = NotifyApps::open(&platform).unwrap();
    assert!(!saved.allowed("org.chat") && !saved.hide_codes() && !saved.show_text());
}
