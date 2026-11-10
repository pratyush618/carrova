mod device_name;

use carrova_core::CodeOffer;
use carrova_core::lan::{ActionCounts, ConflictTarget};
use carrova_core::session::SessionId;
use carrova_platform::{OsKind, SemanticRoot};

use super::*;
use carrova_view::lan::{NearbyPeer, SummaryItemView};

const ESC: &str = "\x1b[2K";

fn code(renewed: bool, fallback: bool) -> CodeView {
    CodeView {
        code: "7-acid-lemon".into(),
        fallback: fallback.then(|| "7-acid-lemon@10.0.0.2:47822".into()),
        renewed,
        expired: false,
        expires_in_secs: 300,
    }
}

#[test]
fn banner_shows_code_once_plus_fallback_ticket() {
    let b = code_banner(&code(false, true), CodeOffer::First);
    assert_eq!(b.matches("7-acid-lemon").count(), 2, "{b}");
    assert!(
        b.contains("If the other machine can't find this one, type: 7-acid-lemon@10.0.0.2:47822"),
        "{b}"
    );
    assert!(!b.contains("Wrong code"), "{b}");
    let b = code_banner(&code(true, false), CodeOffer::Renewed);
    assert_eq!(b.matches("7-acid-lemon").count(), 1, "{b}");
    assert!(b.contains("New code"), "{b}");
}

#[test]
fn banner_says_when_the_code_expires() {
    let b = code_banner(&code(false, false), CodeOffer::First);
    assert!(b.contains("The code expires in 5 min"), "{b}");
    let mut short = code(false, false);
    short.expires_in_secs = 61;
    let b = code_banner(&short, CodeOffer::First);
    assert!(b.contains("The code expires in 2 min"), "{b}");
    short.expires_in_secs = 45;
    let b = code_banner(&short, CodeOffer::First);
    assert!(b.contains("The code expires in 45 s"), "{b}");
}

#[test]
fn banner_after_expiry_says_the_previous_code_expired() {
    let b = code_banner(&code(false, false), CodeOffer::Expired);
    assert!(b.contains("The previous code expired. New code:"), "{b}");
    assert!(!b.contains("can no longer be used"), "{b}");
}

#[test]
fn nearby_senders_are_listed_by_name_escaped() {
    let peers = [
        NearbyPeer {
            device_name: "laptop".into(),
        },
        NearbyPeer {
            device_name: format!("desk{ESC}"),
        },
    ];
    assert_eq!(
        nearby(&peers),
        "Sending nearby: laptop, desk\\u{1b}[2K (type the code shown there)"
    );
    assert_eq!(nearby(&[]), "No machine nearby is offering a code yet.");
}

#[test]
fn pairing_warning_is_loud_and_counts_attempts() {
    let w = pairing_warning(1, 2);
    assert!(
        w.contains("Someone entered a wrong code. If that wasn't you, stop now."),
        "{w}"
    );
    assert!(
        w.contains("Wrong codes so far: 1; the send stops after 2 more."),
        "{w}"
    );
}

#[test]
fn pairing_record_is_the_explicit_code_field() {
    let r = pairing_record(&code(false, true));
    assert_eq!(r["pairing_code"], "7-acid-lemon");
    assert_eq!(r["ticket"], "7-acid-lemon@10.0.0.2:47822");
    assert_eq!(r["renewed"], false);
}

#[test]
fn inventory_escapes_peer_strings() {
    let (version, evil) = (format!("0.1{ESC}"), format!("evil{ESC}"));
    let v = InventoryView {
        os: OsKind::Linux,
        carrova_version: &version,
        apps: vec!["git", &evil],
        free_space: Some(2000),
        device_name: None,
    };
    let s = inventory(&v);
    assert!(!s.contains('\x1b'), "{s}");
    assert!(
        s.contains("Other machine: Linux, Carrova 0.1\\u{1b}[2K, 2.0 kB free"),
        "{s}"
    );
    assert!(s.contains("Apps detected there: git, evil\\u{1b}"), "{s}");
}

#[test]
fn plan_summary_lists_counts_and_escaped_conflicts() {
    let conflicts = [ConflictTarget {
        root: SemanticRoot::Home,
        rel: format!(".gitconfig{ESC}"),
    }];
    let key = carrova_core::ItemKey::from(format!("git/config{ESC}"));
    let v = SummaryView {
        items: vec![SummaryItemView {
            key: &key,
            counts: ActionCounts {
                create: 3,
                replace: 1,
                ..ActionCounts::default()
            },
            conflicts: &conflicts,
            hidden_conflicts: 2,
        }],
        conflicts: 3,
        truncated: true,
    };
    let s = plan_summary(&v);
    assert!(!s.contains('\x1b'), "{s}");
    assert!(
        s.contains("create 3 \u{b7} replace 1 \u{b7} keep-both 0 \u{b7} skip 0 \u{b7} identical 0"),
        "{s}"
    );
    assert!(s.contains("    conflict home:.gitconfig\\u{1b}[2K"), "{s}");
    assert!(s.contains("... and 2 more conflict(s)"), "{s}");
    assert!(s.contains("Conflicts: 3 (-v lists every conflict)"), "{s}");
}

#[test]
fn pause_names_shortfall_and_resume_command() {
    let id = SessionId("20261108-101500-a1b2c3".into());
    let s = pause(&PauseView {
        fs: "/st\x1b",
        shortfall: 3_000_000,
        available: 1000,
        disk: None,
        session: Some(&id),
    });
    assert!(
        s.starts_with("Paused: need 3.0 MB more free on /st\\u{1b} (1.0 kB free)."),
        "{s}"
    );
    assert!(s.contains("transfer resumes automatically"), "{s}");
    assert!(
        s.contains("`carrova receive --resume 20261108-101500-a1b2c3`"),
        "{s}"
    );
}

#[test]
fn pause_names_the_full_disk_when_known() {
    let s = pause(&PauseView {
        fs: "/st",
        shortfall: 3_000_000,
        available: 1000,
        disk: Some("/mnt/data"),
        session: None,
    });
    assert!(
        s.starts_with("Paused: the disk at /mnt/data is full; need 3.0 MB more free on /st"),
        "{s}"
    );
}

#[test]
fn recovery_hints_name_resume_command_and_missing_items() {
    let id = SessionId("20261108-101500-a1b2c3".into());
    let lost = CoreError::PeerUnreachable { waited_secs: 600 };
    let h = recovery_hint(&lost, Some(&id), Role::Export).unwrap();
    assert!(
        h.contains("`carrova send --resume 20261108-101500-a1b2c3`"),
        "{h}"
    );
    let h = recovery_hint(&lost, Some(&id), Role::Import).unwrap();
    assert!(
        h.contains("`carrova receive --resume 20261108-101500-a1b2c3`"),
        "{h}"
    );

    let missing = CoreError::TransferIncomplete {
        items: vec!["git/config".into(), format!("raw:home/x{ESC}").into()],
    };
    let h = recovery_hint(&missing, Some(&id), Role::Import).unwrap();
    assert!(
        h.contains("could not supply git/config, raw:home/x\\u{1b}[2K;"),
        "{h}"
    );
    assert!(
        h.contains("carrova receive --resume 20261108-101500-a1b2c3"),
        "{h}"
    );
    assert!(!h.contains('\x1b'), "{h}");

    let h = recovery_hint(&lost, None, Role::Import).unwrap();
    assert!(
        h.contains("the session is kept: run `carrova receive --resume` when"),
        "{h}"
    );

    assert!(recovery_hint(&CoreError::NotApproved, Some(&id), Role::Import).is_none());
}

#[test]
fn a_cancel_on_the_other_machine_keeps_the_session_resumable() {
    let id = SessionId("20261108-101500-a1b2c3".into());
    let h = recovery_hint(&CoreError::PeerCancelled, Some(&id), Role::Import).unwrap();
    assert!(h.starts_with("the other machine cancelled"), "{h}");
    assert!(
        h.contains("`carrova receive --resume 20261108-101500-a1b2c3`"),
        "{h}"
    );
}
