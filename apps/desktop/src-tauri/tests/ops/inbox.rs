//! What reaches the user when a move ends or a device calls: the bell
//! keeps it, the preferences decide the pop-up and the screen wake.

use carrova_core::{NoticeId, NoticeKind};
use carrova_platform::ScreenLock;
use carrova_view::ipc::{AlertChannel, AlertSettings, ErrorCode, Settings};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::async_runtime::block_on;

use crate::command_error::CommandError;
use crate::events::Emit;
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

/// Waits (up to 5 s) until `done` holds: alerts are delivered off the
/// caller's thread.
fn wait_until(done: impl Fn() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(5), "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn popups(machine: &Machine) -> usize {
    machine.events.recorded.of_kind("popup").len()
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

    wait_until(|| popups(&machine) == 1);
    assert_eq!(kinds(&machine.state), [NoticeKind::MoveFinished]);
    assert_eq!(machine.events.recorded.of_kind("notice").len(), 1);
}

#[test]
fn a_failed_move_is_news_a_cancelled_one_is_not() {
    let machine = Machine::new();
    let failed: Result<(), _> = Err(CommandError::new(ErrorCode::Network, "gone"));
    assert!(inbox::moved(&machine.state, failed).is_err());
    let cancelled: Result<(), _> = Err(CommandError::new(ErrorCode::Cancelled, "stopped"));
    assert!(inbox::moved(&machine.state, cancelled).is_err());

    wait_until(|| !kinds(&machine.state).is_empty());
    std::thread::sleep(Duration::from_millis(100));
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

    wait_until(|| !kinds(&machine.state).is_empty());
    assert_eq!(popups(&machine), 0);
}

#[test]
fn a_send_request_wakes_the_screen_and_hides_the_name_when_locked() {
    let machine = Machine::new();
    machine.platform.set_screen_lock(ScreenLock::Locked);
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
    machine.platform.set_screen_lock(ScreenLock::Unlocked);
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
fn an_unknown_lock_state_hides_the_name_too() {
    let machine = Machine::new();
    inbox::notify(
        &machine.state,
        NoticeKind::SendRequest,
        Some("Ana's laptop".into()),
        "wants to send you files.".into(),
    );
    let popups = machine.events.recorded.of_kind("popup");
    assert_eq!(popups[0]["body"], "A device wants to send you files.");
}

#[test]
fn read_marks_come_back_through_the_list() {
    let machine = Machine::new();
    inbox::moved(&machine.state, Ok::<_, CommandError>(())).unwrap();
    inbox::moved(&machine.state, Ok::<_, CommandError>(())).unwrap();
    wait_until(|| kinds(&machine.state).len() == 2);
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

/// A pop-up that stays open until the test lets it go.
struct HeldPopup {
    release: Mutex<mpsc::Receiver<()>>,
    shown: mpsc::Sender<()>,
}

impl Emit for HeldPopup {
    fn popup(&self, _title: &str, _body: &str) {
        let _ = self.shown.send(());
        let _ = self
            .release
            .lock()
            .unwrap()
            .recv_timeout(Duration::from_secs(5));
    }

    fn emit(&self, _event: &carrova_view::ipc::BridgeEvent) {}
}

#[test]
fn a_slow_pop_up_never_delays_the_move_result() {
    let machine = Machine::new();
    let (release, wait) = mpsc::channel();
    let (shown, popped) = mpsc::channel();
    let state = AppState {
        out: Arc::new(HeldPopup {
            release: Mutex::new(wait),
            shown,
        }),
        ..machine.state.clone()
    };

    let start = Instant::now();
    let result = inbox::moved(&state, Ok::<_, CommandError>(7)).unwrap();

    assert_eq!(result, 7);
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "the result waited for the pop-up"
    );
    popped.recv_timeout(Duration::from_secs(5)).unwrap();
    release.send(()).unwrap();
}
