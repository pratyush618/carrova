use std::path::PathBuf;

use carrova_core::checklist::{Checklist, Skipped};
use carrova_core::session::{Role, RollbackOutcome, SessionId, State};
use carrova_core::store::{SessionRecord, Source};
use carrova_core::{Quarantined, RollbackFailure, RollbackReport};
use carrova_crypto::pairing::CertHash;
use carrova_manifest::Blake3Hash;

use super::ChecklistOf;
use carrova_view::Detail;
const ESC: &str = "\x1b[1A\x1b[2K";

#[test]
fn checklist_headings_follow_role() {
    let c = Checklist {
        moved: vec!["git/config".into()],
        skipped: vec![Skipped {
            what: "ssh/keys".into(),
            reason: "not selected".into(),
        }],
        apps_to_install: Vec::new(),
        rollback_available: false,
    };
    let text = |role| {
        let mut out = Vec::new();
        super::checklist(&mut out, &c, ChecklistOf::Local { session: "s", role }).unwrap();
        String::from_utf8(out).unwrap()
    };
    let export = text(Role::Export);
    assert!(export.contains("Exported (1):"), "{export}");
    assert!(export.contains("Not exported (1):"), "{export}");
    assert!(!export.contains("Moved"), "{export}");
    let import = text(Role::Import);
    assert!(import.contains("Moved (1):"), "{import}");
    assert!(import.contains("Not moved (1):"), "{import}");
}

#[test]
fn receiver_checklist_on_sender_points_rollback_to_other_machine() {
    let c = Checklist {
        moved: vec!["git/config".into()],
        skipped: Vec::new(),
        apps_to_install: vec!["vscode".into()],
        rollback_available: true,
    };
    let mut out = Vec::new();
    super::checklist(&mut out, &c, ChecklistOf::Receiver).unwrap();
    let out = String::from_utf8(out).unwrap();
    assert!(out.contains("Moved (1):"), "{out}");
    assert!(
        out.contains("Install these apps on the other machine"),
        "{out}"
    );
    assert!(
        out.contains("Rollback point kept on the other machine"),
        "{out}"
    );
    assert!(!out.contains("carrova rollback"), "{out}");
}

fn rollback_report() -> RollbackReport {
    RollbackReport {
        restored: (0..40).map(|i| format!("/h/.config/a/f{i}")).collect(),
        removed: vec!["/h/.gitconfig".into()],
        failed: vec![RollbackFailure {
            target: format!("/h/x{ESC}"),
            reason: "permission denied".into(),
        }],
        quarantined: vec![Quarantined {
            target: "/h/.bashrc".into(),
            moved_to: PathBuf::from("/s/rollback-trash/0"),
        }],
    }
}

fn render_rollback(detail: Detail) -> String {
    let mut out = Vec::new();
    super::rollback(&mut out, &rollback_report(), detail).unwrap();
    String::from_utf8(out).unwrap()
}

#[test]
fn rollback_summary_counts_and_lists_only_what_needs_attention() {
    let out = render_rollback(Detail::Summary);
    assert!(
        out.starts_with(
            "Rolled back: 40 restored \u{b7} 1 removed \u{b7} 1 moved aside \u{b7} 1 failed\n"
        ),
        "{out}"
    );
    assert!(!out.contains("f17"), "{out}");
    assert!(
        out.contains("-v lists every restored and removed path"),
        "{out}"
    );
    assert!(
        out.contains("  - /h/.bashrc -> /s/rollback-trash/0"),
        "{out}"
    );
    assert!(
        out.contains("  - /h/x\\u{1b}[1A\\u{1b}[2K: permission denied"),
        "{out}"
    );
    assert!(!out.contains('\x1b'), "{out}");
}

#[test]
fn rollback_step_names_each_outcome_and_escapes_the_path() {
    let line = |o| super::rollback_step(&format!("/h/a{ESC}"), o);
    assert_eq!(
        line(RollbackOutcome::Restored),
        "rollback: restored /h/a\\u{1b}[1A\\u{1b}[2K"
    );
    assert!(line(RollbackOutcome::Removed).starts_with("rollback: removed /h/a"));
    assert!(line(RollbackOutcome::MovedAside).contains("moved aside"));
    assert!(line(RollbackOutcome::Failed).contains("FAILED"));
}

#[test]
fn verbose_rollback_lists_every_path() {
    let out = render_rollback(Detail::Full);
    assert!(out.contains("  restored /h/.config/a/f17"), "{out}");
    assert!(out.contains("  removed /h/.gitconfig"), "{out}");
    assert!(!out.contains("-v lists"), "{out}");
}

fn record(id: &str, source: Option<Source>) -> SessionRecord {
    SessionRecord {
        id: SessionId(id.into()),
        role: Role::Import,
        state: State::Transferring,
        created_at: 0,
        updated_at: 0,
        source,
        approved: true,
        checklist: None,
        error: None,
        bytes_moved: None,
        peer_os: None,
        expires_at: None,
    }
}

#[test]
fn session_list_shows_lan_peer_short_hash_next_to_archives() {
    let list = [
        record(
            "20261108-120000-aaaaaa",
            Some(Source::Lan {
                peer_cert_hash: Some("ab".repeat(32).parse::<CertHash>().unwrap()),
                manifest_fingerprint: Blake3Hash([0; 32]),
                peer_name: None,
            }),
        ),
        record(
            "20261108-110000-bbbbbb",
            Some(Source::Lan {
                peer_cert_hash: None,
                manifest_fingerprint: Blake3Hash([0; 32]),
                peer_name: None,
            }),
        ),
        record(
            "20261108-100000-cccccc",
            Some(Source::Archive {
                path: "/tmp/a.carrova".into(),
                fingerprint: None,
            }),
        ),
    ];
    let mut out = Vec::new();
    super::session_list(&mut out, &list).unwrap();
    let out = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert!(lines[0].contains("LAN peer abababababab"), "{out}");
    assert!(!lines[0].contains("abababababababab"), "{out}");
    assert!(lines[1].contains("LAN (not paired yet)"), "{out}");
    assert!(lines[2].contains("/tmp/a.carrova"), "{out}");
}

#[test]
fn session_list_names_the_peer_with_its_os_and_the_undo_expiry() {
    let mut lan = record(
        "20261108-120000-aaaaaa",
        Some(Source::Lan {
            peer_cert_hash: Some("ab".repeat(32).parse::<CertHash>().unwrap()),
            manifest_fingerprint: Blake3Hash([0; 32]),
            peer_name: Some(format!("laptop{ESC}")),
        }),
    );
    lan.peer_os = Some(carrova_platform::OsKind::Windows);
    lan.expires_at = Some(1_700_000_000);
    let mut out = Vec::new();
    super::session_list(&mut out, &[lan]).unwrap();
    let out = String::from_utf8(out).unwrap();
    assert!(
        out.contains("LAN peer laptop\\u{1b}[1A\\u{1b}[2K (abababababab) · Windows"),
        "{out}"
    );
    assert!(out.contains("[undo until 2023-11-14]"), "{out}");
}
