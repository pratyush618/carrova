//! What reaches the user when a move ends or a device calls: the bell
//! keeps it, the preferences decide the pop-up and the screen wake.

use carrova_core::{NoticeId, NoticeKind};
use carrova_view::ipc::{AlertChannel, AlertSettings, ErrorCode, Settings};
use tauri::async_runtime::block_on;

use crate::command_error::CommandError;
use crate::ops::{inbox, settings};
use crate::state::AppState;
use crate::testing::Machine;

fn alerts(machine: &Machine, alerts: AlertSettings) {
    let settings = Settings {
        alerts,
        ..Settings::default()
    };
    block_on(settings::save(&machine.state, settings)).unwrap();
}

fn kinds(state: &AppState) -> Vec<NoticeKind> {
    block_on(inbox::list(state))
        .unwrap()
        .iter()
        .map(|n| n.kind)
        .collect()
}

#[test]
fn a_finished_move_is_kept_announced_and_popped_up() {
    let machine = Machine::new();
    let done: Result<(), CommandError> = Ok(());
    inbox::moved(&machine.state, done).unwrap();

    assert_eq!(kinds(&machine.state), [NoticeKind::MoveFinished]);
    assert_eq!(machine.events.recorded.of_kind("notice").len(), 1);
    assert_eq!(machine.events.recorded.of_kind("popup").len(), 1);
}

#[test]
fn a_failed_move_is_news_a_cancelled_one_is_not() {
    let machine = Machine::new();
    let failed: Result<(), _> = Err(CommandError::new(ErrorCode::Network, "gone"));
    assert!(inbox::moved(&machine.state, failed).is_err());
    let cancelled: Result<(), _> = Err(CommandError::new(ErrorCode::Cancelled, "stopped"));
    assert!(inbox::moved(&machine.state, cancelled).is_err());

    assert_eq!(kinds(&machine.state), [NoticeKind::MoveFailed]);
}

#[test]
fn a_bell_only_alert_is_kept_without_a_pop_up() {
    let machine = Machine::new();
    alerts(
        &machine,
        AlertSettings {
            moves: AlertChannel::BellOnly,
            ..AlertSettings::default()
        },
    );
    inbox::moved(&machine.state, Ok::<_, CommandError>(())).unwrap();

    assert_eq!(kinds(&machine.state), [NoticeKind::MoveFinished]);
    assert!(machine.events.recorded.of_kind("popup").is_empty());
}

#[test]
fn a_send_request_wakes_the_screen_and_hides_the_name_when_locked() {
    let machine = Machine::new();
    machine.platform.set_screen_locked(true);
    inbox::notify(
        &machine.state,
        NoticeKind::SendRequest,
        Some("Ana's laptop".into()),
        "wants to send you files.".into(),
    );

    assert_eq!(machine.platform.wakes(), 1);
    let popups = machine.events.recorded.of_kind("popup");
    assert_eq!(popups[0]["body"], "A device wants to send you files.");
}

#[test]
fn an_unlocked_pop_up_names_the_device() {
    let machine = Machine::new();
    inbox::notify(
        &machine.state,
        NoticeKind::SendRequest,
        Some("Ana's laptop".into()),
        "wants to send you files.".into(),
    );
    let popups = machine.events.recorded.of_kind("popup");
    assert_eq!(popups[0]["body"], "Ana's laptop wants to send you files.");
}

#[test]
fn read_marks_come_back_through_the_list() {
    let machine = Machine::new();
    inbox::moved(&machine.state, Ok::<_, CommandError>(())).unwrap();
    inbox::moved(&machine.state, Ok::<_, CommandError>(())).unwrap();
    let ids: Vec<NoticeId> = block_on(inbox::list(&machine.state))
        .unwrap()
        .iter()
        .map(|n| n.id)
        .collect();

    block_on(inbox::mark_read(&machine.state, Some(vec![ids[0]]))).unwrap();
    let unread = |state: &AppState| {
        block_on(inbox::list(state))
            .unwrap()
            .iter()
            .filter(|n| !n.read)
            .count()
    };
    assert_eq!(unread(&machine.state), 1);
    block_on(inbox::mark_read(&machine.state, None)).unwrap();
    assert_eq!(unread(&machine.state), 0);
}
