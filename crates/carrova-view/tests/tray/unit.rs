use super::*;
use crate::devices::VisibilityChoice;
use crate::ipc::AlertSettings;

/// Nothing waiting or running, visible to known devices, none counted.
fn idle() -> TrayModel {
    TrayModel {
        request: None,
        paused: false,
        moving: false,
        visible_to: VisibilityChoice::Known,
        nearby_count: None,
    }
}

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
        moving: false,
        ..idle()
    };
    assert_eq!(
        ids(&model),
        [
            "review",
            "decline",
            "open",
            "send",
            "nearby",
            "pause",
            "visibility",
            "quit"
        ]
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
        moving: false,
        ..idle()
    };
    assert_eq!(
        ids(&model),
        ["open", "send", "nearby", "pause", "visibility", "quit"]
    );
    assert!(!tray_menu(&model)[0].separated);
}

#[test]
fn tray_menu_offers_to_resume_paused_alerts() {
    let label = |paused| {
        tray_menu(&TrayModel {
            request: None,
            paused,
            ..idle()
        })
        .into_iter()
        .find(|item| item.id == "pause")
        .map(|item| item.label)
    };
    assert_eq!(label(false).as_deref(), Some("Pause alerts for 1 hour"));
    assert_eq!(label(true).as_deref(), Some("Resume alerts"));
}

#[test]
fn tray_menu_leads_with_a_running_move() {
    let model = TrayModel {
        request: None,
        paused: false,
        moving: true,
        ..idle()
    };
    assert_eq!(
        ids(&model),
        [
            "moving",
            "open",
            "send",
            "nearby",
            "pause",
            "visibility",
            "quit"
        ]
    );
    let menu = tray_menu(&model);
    assert_eq!(menu[0].label, "Moving files…");
    assert!(!menu[0].enabled, "a status line, not an action");
    assert!(menu[1].enabled && menu[1].separated);
}

fn item(model: &TrayModel, id: &str) -> TrayItem {
    tray_menu(model).into_iter().find(|i| i.id == id).unwrap()
}

#[test]
fn tray_counts_the_devices_nearby_when_known() {
    assert_eq!(item(&idle(), "nearby").label, "Nearby");
    let counted = TrayModel {
        nearby_count: Some(3),
        ..idle()
    };
    assert_eq!(item(&counted, "nearby").label, "Nearby · 3 here");
}

#[test]
fn tray_visibility_submenu_ticks_the_current_choice() {
    let model = TrayModel {
        visible_to: VisibilityChoice::Everyone,
        ..idle()
    };
    let visibility = item(&model, "visibility");
    assert_eq!(visibility.label, "Visible to");
    let choices: Vec<_> = visibility
        .children
        .iter()
        .map(|c| (c.id, c.label.as_str(), c.checked))
        .collect();
    assert_eq!(
        choices,
        [
            ("visible_no_one", "No one", Some(false)),
            ("visible_known", "Known devices", Some(false)),
            ("visible_everyone", "Everyone for 10 minutes", Some(true)),
        ]
    );
    for (id, choice) in [
        ("visible_no_one", VisibilityChoice::NoOne),
        ("visible_known", VisibilityChoice::Known),
        ("visible_everyone", VisibilityChoice::Everyone),
    ] {
        assert_eq!(tray_visibility(id), Some(choice));
    }
    assert_eq!(tray_visibility("quit"), None);
}

#[test]
fn tray_tooltip_says_a_move_is_running() {
    let idle = TrayModel {
        request: None,
        paused: false,
        moving: false,
        ..idle()
    };
    assert_eq!(tray_tooltip(&idle), "Carrova");
    let moving = TrayModel {
        moving: true,
        ..idle
    };
    assert_eq!(tray_tooltip(&moving), "Carrova · Moving files");
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
    assert_eq!(
        close_action(true, true, false, true),
        CloseAction::CloseToTray
    );
}

#[test]
fn close_without_tray_minimises() {
    assert_eq!(
        close_action(true, false, false, true),
        CloseAction::Minimise
    );
    assert_eq!(close_action(true, false, true, true), CloseAction::Minimise);
}

#[test]
fn close_with_keep_running_off_exits() {
    assert_eq!(close_action(false, true, false, true), CloseAction::Exit);
    assert_eq!(close_action(false, false, true, true), CloseAction::Exit);
}

/// A move is running: the window only hides, so reopening shows the flow
/// exactly as it was.
#[test]
fn reopen_during_transfer_restores_flow() {
    assert_eq!(
        close_action(true, true, true, true),
        CloseAction::HideToTray
    );
}

/// Hidden while busy, then the last move ended: deciding again frees the
/// webview.
#[test]
fn idle_after_a_busy_close_frees_the_window() {
    let busy = close_action(true, true, true, true);
    let idle = close_action(true, true, false, true);
    assert_eq!(
        (busy, idle),
        (CloseAction::HideToTray, CloseAction::CloseToTray)
    );
}

/// "Use as little as possible" off: the window is kept (hidden) even idle.
#[test]
fn closing_keeps_the_window_when_lean_is_off() {
    assert_eq!(
        close_action(true, true, false, false),
        CloseAction::HideToTray
    );
    assert_eq!(
        close_action(true, true, true, false),
        CloseAction::HideToTray
    );
    assert_eq!(
        close_action(true, false, false, false),
        CloseAction::Minimise
    );
    assert_eq!(close_action(false, true, false, false), CloseAction::Exit);
}
