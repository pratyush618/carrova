//! The engine results a desktop front-end receives over IPC serialise to
//! JSON (and the requests it sends deserialise). Secrets (`Ticket`,
//! `Passphrase`) deliberately have no serde: front-ends send pairing-code
//! text and the bridge parses it.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

use carrova_core::session::PauseReason;
use carrova_core::{
    ArchiveSummary, Checklist, CodeOffer, Event, ItemKey, Phase, Quarantined, RollbackFailure,
    RollbackOutcome, RollbackReport, ScanOptions, SendOutcome, SendReport,
};
use serde_json::json;

#[test]
fn rollback_report_serialises_failures_and_quarantine_as_named_fields() {
    let report = RollbackReport {
        restored: vec!["/h/a".into()],
        removed: vec!["/h/b".into()],
        failed: vec![RollbackFailure {
            target: "/h/c".into(),
            reason: "changed since the import".into(),
        }],
        quarantined: vec![Quarantined {
            target: "/h/d".into(),
            moved_to: PathBuf::from("/s/rollback-trash/1-0-d"),
        }],
    };
    assert_eq!(
        serde_json::to_value(&report).unwrap(),
        json!({
            "restored": ["/h/a"],
            "removed": ["/h/b"],
            "failed": [{"target": "/h/c", "reason": "changed since the import"}],
            "quarantined": [{"target": "/h/d", "moved_to": "/s/rollback-trash/1-0-d"}],
        })
    );
}

#[test]
fn send_report_serialises_with_a_snake_case_outcome() {
    let report = SendReport {
        checklist: Checklist::default(),
        chunks: 3,
        bytes: 42,
        outcome: SendOutcome::DryRun,
    };
    let value = serde_json::to_value(&report).unwrap();
    assert_eq!(value["outcome"], "dry_run");
    assert_eq!(value["bytes"], 42);
    assert_eq!(serde_json::to_value(CodeOffer::Renewed).unwrap(), "renewed");
}

#[test]
fn archive_summary_serialises_its_counts() {
    let summary = ArchiveSummary {
        chunks: 2,
        bytes: 1024,
    };
    assert_eq!(
        serde_json::to_value(summary).unwrap(),
        json!({"chunks": 2, "bytes": 1024})
    );
}

#[test]
fn scan_options_deserialise_from_a_front_end_request() {
    let opts: ScanOptions =
        serde_json::from_value(json!({"include_undetected": true, "unrecognised": false})).unwrap();
    assert_eq!(
        opts,
        ScanOptions {
            include_undetected: true,
            unrecognised: false,
        }
    );
}

#[test]
fn events_round_trip_through_json() {
    let events = [
        Event::Progress {
            phase: Phase::Transfer,
            done: 1,
            total: 2,
        },
        Event::RollbackStep {
            target: "/h/a".into(),
            outcome: RollbackOutcome::MovedAside,
        },
        Event::TransferPaused {
            reason: PauseReason::LowDiskSpace {
                fs: "/s".into(),
                available: 1,
                needed: 2,
                disk: None,
            },
        },
        Event::ItemCaptured {
            item: ItemKey::from("app/config"),
            files: 1,
            bytes: 2,
        },
    ];
    for event in events {
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(serde_json::from_str::<Event>(&json).unwrap(), event);
    }
}
