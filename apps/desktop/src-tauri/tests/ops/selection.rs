//! The sending side's selection through the bridge: the user's edits are
//! applied by the engine's rules, and the engine (not the bridge) refuses
//! to export or send an unusable selection.

use carrova_core::{ItemKey, KdfStrength, PickResult, Selection, SelectionEdit, SessionId};
use carrova_view::ipc::{ErrorCode, ScanReply};
use tauri::async_runtime::block_on;

use crate::ops::send;
use crate::testing::{Machine, PASS};

/// A sender with the app's config and a sensitive key, scanned.
fn scanned() -> (Machine, SessionId, Selection) {
    let sender = Machine::new();
    sender.put(".config/app/settings.toml", b"x");
    sender.put(".ssh/id_ed25519", b"secret");
    let scan = block_on(send::scan(&sender.state)).unwrap();
    let result = scan.result();
    (sender, result.session, result.selection.clone())
}

fn keys() -> ItemKey {
    "app/keys".into()
}

fn tick_keys() -> SelectionEdit {
    SelectionEdit::Item {
        item: keys(),
        included: true,
    }
}

fn select(
    sender: &Machine,
    id: &SessionId,
    selection: Selection,
    edits: Vec<SelectionEdit>,
) -> ScanReply {
    block_on(send::select(&sender.state, id, selection, edits)).unwrap()
}

fn export(sender: &Machine, id: &SessionId) -> Result<(), ErrorCode> {
    block_on(send::export_archive(
        &sender.state,
        id,
        sender.scratch("move.carrova"),
        PASS.into(),
        KdfStrength::default(),
    ))
    .map(|_| ())
    .map_err(|e| e.bridge().code)
}

#[test]
fn a_scan_hands_the_suggested_selection_to_the_engine() {
    let (sender, id, _) = scanned();
    assert_eq!(export(&sender, &id), Ok(()));
}

#[test]
fn edits_are_applied_by_the_engine_rules() {
    let (sender, id, selection) = scanned();
    let confirm = SelectionEdit::ConfirmSensitive {
        item: keys(),
        confirmed: true,
    };
    let reply = select(&sender, &id, selection, vec![tick_keys(), confirm]);
    let result = reply.result();
    let choice = &result.selection.items[&keys()];
    assert!(choice.included && choice.confirmed_sensitive);
    assert!(result.view.issues.is_empty());
    assert_eq!(export(&sender, &id), Ok(()));
}

#[test]
fn a_refused_selection_is_shown_with_its_issues_and_not_exported() {
    let (sender, id, selection) = scanned();
    let reply = select(&sender, &id, selection, vec![tick_keys()]);
    let result = reply.result();
    assert!(result.selection.items[&keys()].included);
    assert_eq!(result.view.issues[0].item, "app/keys");
    assert_eq!(export(&sender, &id), Err(ErrorCode::InvalidSelection));
}

#[test]
fn a_refused_send_keeps_the_session_selecting() {
    let (sender, id, selection) = scanned();
    let refused = select(&sender, &id, selection, vec![tick_keys()]);
    let err = block_on(send::run_send(&sender.state, &id)).unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::InvalidSelection);

    let untick = SelectionEdit::Item {
        item: keys(),
        included: false,
    };
    let fixed = refused.result().selection.clone();
    let reply = select(&sender, &id, fixed, vec![untick]);
    assert!(reply.result().view.issues.is_empty());
    assert_eq!(export(&sender, &id), Ok(()));
}

#[test]
fn an_edit_naming_an_unknown_item_changes_nothing() {
    let (sender, id, selection) = scanned();
    let unknown = SelectionEdit::Item {
        item: "nope/nothing".into(),
        included: true,
    };
    let err = block_on(send::select(&sender.state, &id, selection, vec![unknown])).unwrap_err();
    assert_eq!(err.bridge().code, ErrorCode::InvalidSelection);
    assert_eq!(export(&sender, &id), Ok(()), "the suggestion still holds");
}

#[test]
fn the_meter_rates_a_passphrase_as_export_does() {
    use carrova_view::passphrase::{PassphraseStrength, PassphraseUse};
    assert_eq!(
        send::passphrase_strength("hunter2".into()),
        PassphraseStrength::Weak
    );
    let strength = send::passphrase_strength(PASS.into());
    assert!(strength >= PassphraseUse::NewArchive.minimum());
}

#[test]
fn manual_pick_then_export_writes_archive() {
    let sender = Machine::new();
    sender.put(".config/app/settings.toml", b"x");
    let opened = block_on(send::start_manual(&sender.state)).unwrap();
    let result = opened.result();
    assert!(result.report.apps.is_empty(), "no app scan in Manual mode");
    assert!(result.selection.items.is_empty());

    let folder = sender.home.join(".config/app");
    let picked = block_on(send::pick(
        &sender.state,
        &result.session,
        result.selection.clone(),
        vec![folder],
    ))
    .unwrap();
    let view = picked.picked();
    assert!(matches!(view.outcomes[0].result, PickResult::Added { .. }));
    assert!(view.scan.view.issues.is_empty());
    assert_eq!(export(&sender, &result.session), Ok(()));
    assert!(sender.scratch("move.carrova").is_file());
}

#[test]
fn scan_apps_after_manual_returns_suggestion() {
    let sender = Machine::new();
    sender.put(".config/app/settings.toml", b"x");
    let id = block_on(send::start_manual(&sender.state))
        .unwrap()
        .result()
        .session;
    let scanned = block_on(send::scan_apps(&sender.state, &id)).unwrap();
    let result = scanned.result();
    assert_eq!(result.report.apps.len(), 1);
    assert!(result.selection.items[&ItemKey::from("app/config")].included);
}
