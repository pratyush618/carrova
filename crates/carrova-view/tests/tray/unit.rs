use super::*;
use crate::ipc::AlertSettings;

fn ids(model: &TrayModel) -> Vec<&'static str> {
    tray_menu(model).iter().map(|item| item.id).collect()
}

#[test]
fn tray_menu_puts_request_first() {
    let model = TrayModel {
        request: Some(TrayRequest {
            peer: "Ana's laptop".into(),
        }),
        paused: false,
    };
    assert_eq!(
        ids(&model),
        ["review", "decline", "open", "send", "pause", "quit"]
    );
    let menu = tray_menu(&model);
    assert!(menu[0].label.contains("Ana's laptop"), "{:?}", menu[0]);
    // The request stands apart from the everyday items.
    assert!(menu[2].separated);
}

#[test]
fn tray_menu_without_request_starts_with_open() {
    let model = TrayModel {
        request: None,
        paused: false,
    };
    assert_eq!(ids(&model), ["open", "send", "pause", "quit"]);
    assert!(!tray_menu(&model)[0].separated);
}

#[test]
fn tray_menu_offers_to_resume_paused_alerts() {
    let label = |paused| {
        tray_menu(&TrayModel {
            request: None,
            paused,
        })
        .into_iter()
        .find(|item| item.id == "pause")
        .map(|item| item.label)
    };
    assert_eq!(label(false).as_deref(), Some("Pause alerts for 1 hour"));
    assert_eq!(label(true).as_deref(), Some("Resume alerts"));
}

#[test]
fn pause_toggle_pauses_for_an_hour_then_resumes() {
    let now = 1_000;
    let mut alerts = AlertSettings::default();
    assert!(!alerts_paused(&alerts, now));

    alerts.paused_until = toggled_pause(&alerts, now);
    assert_eq!(alerts.paused_until, Some(now + 3_600));
    assert!(alerts_paused(&alerts, now));

    assert_eq!(toggled_pause(&alerts, now), None);
}

#[test]
fn a_pause_that_ran_out_is_over() {
    let alerts = AlertSettings {
        paused_until: Some(500),
        ..AlertSettings::default()
    };
    assert!(!alerts_paused(&alerts, 500));
    assert_eq!(toggled_pause(&alerts, 500), Some(500 + 3_600));
}

#[test]
fn close_with_keep_running_and_tray_hides() {
    assert_eq!(close_action(true, true), CloseAction::HideToTray);
}

#[test]
fn close_without_tray_minimises() {
    assert_eq!(close_action(true, false), CloseAction::Minimise);
}

#[test]
fn close_with_keep_running_off_exits() {
    assert_eq!(close_action(false, true), CloseAction::Exit);
    assert_eq!(close_action(false, false), CloseAction::Exit);
}
