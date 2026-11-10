//! Settings, recipes, pairing answers and session bookkeeping through the
//! bridge.

use carrova_core::SessionId;
use carrova_view::ipc::{ErrorCode, RecipeOrigin, RecipeTier, Settings, Theme};
use tauri::async_runtime::block_on;

use crate::ops::{receive, recipes, send, sessions, settings};
use crate::testing::Machine;

fn unknown() -> SessionId {
    SessionId("20261108-101500-a1b2c3".into())
}

#[test]
fn settings_round_trip() {
    let machine = Machine::new();
    assert_eq!(
        block_on(settings::load(&machine.state)).unwrap(),
        Settings::default()
    );
    let mine = Settings {
        theme: Some(Theme::Light),
        reduce_motion: Some(true),
        ..Settings::default()
    };
    block_on(settings::save(&machine.state, mine.clone())).unwrap();
    assert_eq!(block_on(settings::load(&machine.state)).unwrap(), mine);
}

#[test]
fn the_catalog_lists_recipes_with_detection_and_tier() {
    let machine = Machine::new();
    let catalog = block_on(recipes::catalog(&machine.state)).unwrap();
    assert_eq!(catalog.len(), 1);
    let app = &catalog[0];
    assert_eq!((app.id.as_str(), app.detected), ("app", true));
    assert_eq!(app.tier, RecipeTier::Unverified, "a dev tree is unverified");
    let keys = app.items.iter().find(|i| i.id == "keys").unwrap();
    assert!(keys.sensitive && !keys.default);
}

#[test]
fn status_reports_a_dev_tree_as_its_origin() {
    let machine = Machine::new();
    let status = block_on(recipes::status(&machine.state)).unwrap();
    assert_eq!(status.origin, RecipeOrigin::DevDir);
    assert_eq!(status.created, None);
    // Effective tiers: a dev tree's recipes are unverified, in neither tier.
    assert_eq!((status.verified, status.community), (0, 0));
}

#[test]
fn answering_with_no_open_question_fails() {
    let machine = Machine::new();
    let err = send::answer_pairing(&machine.state, &unknown(), true).unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Session);
}

#[test]
fn commands_on_unknown_sessions_fail_with_a_session_error() {
    let machine = Machine::new();
    let state = &machine.state;
    let errors = [
        sessions::cancel(state, &unknown()).unwrap_err(),
        block_on(receive::approve(state, &unknown())).unwrap_err(),
        block_on(send::run_send(state, &unknown())).unwrap_err(),
        block_on(sessions::resume(state, &unknown(), None)).unwrap_err(),
    ];
    for err in errors {
        assert_eq!(err.bridge().code, ErrorCode::Session, "{err:?}");
    }
}

#[test]
fn a_malformed_code_is_refused_without_echoing_it() {
    let machine = Machine::new();
    let err = block_on(receive::connect(&machine.state, "7-acid-lemonade".into())).unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::BadCode);
    let shown = format!("{} {:?}", err.bridge().message, err.bridge().hint);
    assert!(
        !shown.contains("acid") && !shown.contains("lemonade"),
        "{shown}"
    );
}

#[test]
fn a_dismissed_session_is_closed_but_stays_in_history() {
    let machine = Machine::new();
    let scanned = block_on(send::scan(&machine.state)).unwrap();
    let id = scanned.result().session;
    block_on(sessions::dismiss(&machine.state, &id)).unwrap();
    let history = block_on(sessions::list(&machine.state)).unwrap();
    assert!(history.iter().any(|r| r.id == id));
    let err = block_on(send::suggest(
        &machine.state,
        &id,
        carrova_core::Preset::None,
    ))
    .unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::Session);
}
