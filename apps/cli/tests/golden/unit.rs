//! Golden tests: the exact text (and JSON) every renderer prints for a set
//! of representative fixtures. They pin CLI output byte for byte across
//! refactors. Expected output lives in `tests/golden/<name>.txt`; after an
//! intended output change, regenerate with `CARROVA_BLESS=1 cargo test -p
//! carrova-cli golden` and review the diff.

mod fixtures;
mod recipes;
mod reports;

use std::fmt::Write as _;
use std::path::Path;

use carrova_core::RollbackReport;
use carrova_core::checklist::Checklist;
use carrova_core::lan::{
    ActionCounts, ConflictTarget, Inventory, InventoryApp, ItemPlanSummary, PlanSummary,
};
use carrova_core::select::Selection;
use carrova_core::session::{PauseReason, Role, RollbackOutcome, SessionId};
use carrova_core::{CodeOffer, CoreError};
use carrova_platform::{OsKind, SemanticRoot};

use crate::render::lan as lan_render;
use crate::render::plan::plan;
use crate::render::scan::scan_report;
use crate::render::selection::selection;
use crate::render::session::{
    ChecklistOf, checklist, rollback, rollback_step, session_list, session_show,
};
use carrova_view::Detail;
use carrova_view::lan::{code_view, inventory_view, pause_view, summary_view};
use carrova_view::selection::selection_view;

use fixtures::*;

/// Compare `actual` with `tests/golden/<name>.txt` (or write it with
/// `CARROVA_BLESS=1`).
fn golden(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!("tests/golden/{name}.txt"));
    if std::env::var_os("CARROVA_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, actual).unwrap();
        return;
    }
    let expected = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("{}: {e} (bless with CARROVA_BLESS=1)", path.display()));
    assert_eq!(actual, expected, "golden {name} differs");
}

fn text(f: impl FnOnce(&mut Vec<u8>) -> std::io::Result<()>) -> String {
    let mut out = Vec::new();
    f(&mut out).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn golden_plan_summary() {
    let (p, m) = sample_plan();
    golden(
        "plan_summary",
        &text(|o| plan(o, &p, Some(&m), &needs(), Detail::Summary)),
    );
}

#[test]
fn golden_plan_verbose_with_options() {
    let (mut p, m) = sample_plan();
    p.options.keep_group_write = true;
    p.options.import_trust = true;
    p.options.confirmed_sensitive = vec!["gnupg/keys".into()];
    p.handlers[0].import_trust = true;
    golden(
        "plan_verbose_options",
        &text(|o| plan(o, &p, Some(&m), &needs(), Detail::Full)),
    );
    p.options.items = Some(vec!["ssh/keys".into()]);
    golden(
        "plan_explicit_items",
        &text(|o| plan(o, &p, Some(&m), &[], Detail::Summary)),
    );
}

#[test]
fn golden_plan_empty_without_manifest() {
    let (mut p, _) = sample_plan();
    p.actions.clear();
    p.handlers.clear();
    p.missing_apps.clear();
    p.running_apps.clear();
    p.blockers.clear();
    p.warnings.clear();
    golden(
        "plan_empty",
        &text(|o| plan(o, &p, None, &[], Detail::Summary)),
    );
}

#[test]
fn golden_scan_report() {
    let r = scan_fixture();
    golden("scan", &text(|o| scan_report(o, &r)));
    let mut none = scan_fixture();
    none.apps.retain(|a| !a.detected);
    none.unrecognised.clear();
    none.platform.free_space = None;
    none.platform.snapshot = None;
    none.platform.os = OsKind::MacOs;
    none.platform.roots.clear();
    golden("scan_nothing_detected", &text(|o| scan_report(o, &none)));
}

#[test]
fn golden_selection() {
    let scan = scan_fixture();
    let mut sel = Selection::default();
    let mut git = choice(true);
    git.suggested = true;
    git.excluded_paths = vec![format!("home:.gitconfig{ESC}")];
    sel.items.insert("git/config".into(), git);
    let mut keys = choice(false);
    keys.sensitive = true;
    sel.items.insert("ssh/keys".into(), keys);
    let mut confirmed = choice(true);
    confirmed.sensitive = true;
    confirmed.confirmed_sensitive = true;
    sel.items.insert("gnupg/keys".into(), confirmed);
    let mut forced = choice(true);
    forced.unmovable = Some(format!("bound{ESC}"));
    forced.force_unmovable = true;
    sel.items.insert("ssh/machine-id".into(), forced);
    let mut unforced = choice(true);
    unforced.unmovable = Some("bound".into());
    sel.items.insert("zsh/history".into(), unforced);
    sel.items
        .insert("raw:user_config/oddtool".into(), choice(true));
    sel.items
        .insert("raw:user_data/locked".into(), choice(false));
    golden(
        "selection",
        &text(|o| selection(o, &selection_view(&sel, &scan))),
    );
}

#[test]
fn golden_checklists() {
    let mut s = String::new();
    for (of, c) in [
        (
            ChecklistOf::Local {
                session: "s1",
                role: Role::Export,
            },
            sample_checklist(false),
        ),
        (
            ChecklistOf::Local {
                session: "s2",
                role: Role::Import,
            },
            sample_checklist(true),
        ),
        (ChecklistOf::Receiver, sample_checklist(true)),
        (
            ChecklistOf::Receiver,
            Checklist {
                moved: Vec::new(),
                skipped: Vec::new(),
                apps_to_install: Vec::new(),
                rollback_available: false,
            },
        ),
    ] {
        s.push_str(&text(|o| checklist(o, &c, of)));
        s.push_str("--\n");
    }
    golden("checklists", &s);
}

#[test]
fn golden_rollback() {
    let r = rollback_report();
    let mut s = text(|o| rollback(o, &r, Detail::Summary));
    s.push_str("--\n");
    s.push_str(&text(|o| rollback(o, &r, Detail::Full)));
    s.push_str("--\n");
    let empty = RollbackReport {
        restored: Vec::new(),
        removed: Vec::new(),
        failed: Vec::new(),
        quarantined: Vec::new(),
    };
    s.push_str(&text(|o| rollback(o, &empty, Detail::Summary)));
    s.push_str("--\n");
    s.push_str(&serde_json::to_string_pretty(&serde_json::to_value(&r).unwrap()).unwrap());
    s.push('\n');
    for o in [
        RollbackOutcome::Restored,
        RollbackOutcome::Removed,
        RollbackOutcome::MovedAside,
        RollbackOutcome::Failed,
    ] {
        s.push_str(&rollback_step(&format!("/h/a{ESC}"), o));
        s.push('\n');
    }
    golden("rollback", &s);
}

#[test]
fn golden_sessions() {
    let list = sessions_fixture();
    let mut s = text(|o| session_list(o, &list));
    s.push_str(&text(|o| session_list(o, &[])));
    for r in &list {
        s.push_str("--\n");
        s.push_str(&text(|o| session_show(o, r)));
    }
    golden("sessions", &s);
}

#[test]
fn golden_lan_code_and_warnings() {
    let mut s = String::new();
    for (t, offer) in [
        ("7-acid-lemon@192.168.1.20:47822", CodeOffer::First),
        ("7-acid-lemon", CodeOffer::Renewed),
    ] {
        let v = code_view(&ticket(t), offer);
        s.push_str(&lan_render::code_banner(&v));
        s.push_str("\n--\n");
        s.push_str(&serde_json::to_string(&lan_render::pairing_record(&v)).unwrap());
        s.push_str("\n--\n");
    }
    s.push_str(&lan_render::pairing_warning(1, 2));
    s.push('\n');
    s.push_str(lan_render::NEW_CODE_QUESTION);
    s.push('\n');
    s.push_str(lan_render::NEW_CODE_NOT_ASKED);
    s.push('\n');
    golden("lan_code", &s);
}

#[test]
fn golden_lan_peer_views() {
    let mut inv = Inventory {
        os: OsKind::Linux,
        carrova_version: format!("0.1{ESC}"),
        apps: ["zsh", "git"]
            .map(|id| InventoryApp {
                id: id.into(),
                version: None,
            })
            .into(),
        free_space: Some(2048),
        device_name: None,
    };
    let mut s = lan_render::inventory(&inventory_view(&inv));
    s.push('\n');
    inv.apps.clear();
    inv.free_space = None;
    inv.os = OsKind::MacOs;
    s.push_str(&lan_render::inventory(&inventory_view(&inv)));
    s.push('\n');
    inv.os = OsKind::Windows;
    s.push_str(&lan_render::inventory(&inventory_view(&inv)));
    s.push('\n');

    let summary = PlanSummary {
        items: vec![
            ItemPlanSummary {
                item: format!("git/config{ESC}").into(),
                counts: ActionCounts {
                    create: 3,
                    replace: 7,
                    ..ActionCounts::default()
                },
                conflicts: (0..7)
                    .map(|i| ConflictTarget {
                        root: SemanticRoot::Home,
                        rel: format!(".f{i}{ESC}"),
                    })
                    .collect(),
            },
            ItemPlanSummary {
                item: "zsh/rc".into(),
                counts: ActionCounts {
                    identical: 1,
                    ..ActionCounts::default()
                },
                conflicts: Vec::new(),
            },
        ],
    };
    for detail in [Detail::Summary, Detail::Full] {
        s.push_str(&lan_render::plan_summary(&summary_view(&summary, detail)));
        s.push('\n');
    }
    let empty = PlanSummary { items: Vec::new() };
    s.push_str(&lan_render::plan_summary(&summary_view(
        &empty,
        Detail::Summary,
    )));
    s.push('\n');

    let reason = PauseReason::LowDiskSpace {
        fs: format!("/st{ESC}"),
        available: 1024,
        needed: 3 * 1024 * 1024 + 1024,
    };
    let id = SessionId("20261108-101500-a1b2c3".into());
    s.push_str(&lan_render::pause(&pause_view(&reason, Some(&id)).unwrap()));
    s.push('\n');
    s.push_str(&lan_render::pause(&pause_view(&reason, None).unwrap()));
    s.push('\n');
    golden("lan_peer", &s);
}

#[test]
fn golden_lan_recovery_hints() {
    let id = SessionId("20261108-101500-a1b2c3".into());
    let errors = [
        CoreError::PeerUnreachable { waited_secs: 600 },
        CoreError::TransferIncomplete {
            items: vec!["git/config".into(), format!("raw:home/x{ESC}").into()],
        },
        CoreError::PeerAborted(format!("bye{ESC}")),
        CoreError::NotApproved,
    ];
    let mut s = String::new();
    for e in &errors {
        for role in [Role::Export, Role::Import] {
            for session in [Some(&id), None] {
                let hint = lan_render::recovery_hint(e, session, role);
                let _ = writeln!(s, "{hint:?}");
            }
        }
    }
    golden("lan_hints", &s);
}
