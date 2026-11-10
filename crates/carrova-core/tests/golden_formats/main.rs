//! Golden formats: bytes that leave the engine (persisted `session.json`,
//! `log.jsonl` events, LAN wire messages) and user-visible texts. These pin
//! the formats across refactors; a change here is a format change and needs
//! a migration (persisted data) or a protocol version bump (wire).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fmt::Write as _;

use carrova_core::lan::{
    AbortReason, ActionCounts, ConflictTarget, Inventory, InventoryApp, ItemPlanSummary, LanMsg,
    PlanSummary, ReceiverStage,
};
use carrova_core::session::SessionId;
use carrova_core::{
    Checklist, CoreError, Corruption, Event, JournalProblem, PauseReason, Phase, ResumeKeyProblem,
    RollbackOutcome, SessionProblem, SessionRecord, Skipped, State,
};
use carrova_platform::{OsKind, SemanticRoot};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

fn cbor(msg: &LanMsg) -> String {
    let mut out = Vec::new();
    ciborium::into_writer(msg, &mut out).unwrap();
    hex(&out)
}

fn checklist() -> Checklist {
    Checklist {
        moved: vec!["git/config".into()],
        skipped: vec![Skipped {
            what: "ssh/keys".into(),
            reason: "not selected".into(),
        }],
        apps_to_install: vec!["Git".into()],
        rollback_available: true,
    }
}

// ------------------------------------------------------------------ session.json

/// `session.json` as written by the store (pretty JSON), archive import.
const ARCHIVE_RECORD: &str = r#"{
  "id": "20261108-101500-a1b2c3",
  "role": "import",
  "state": "done",
  "created_at": 1793000000,
  "updated_at": 1793000100,
  "source": {
    "kind": "archive",
    "path": "/home/u/backup.carrova",
    "fingerprint": {
      "header": "0101010101010101010101010101010101010101010101010101010101010101",
      "manifest": "0202020202020202020202020202020202020202020202020202020202020202"
    }
  },
  "approved": true,
  "checklist": {
    "moved": [
      "git/config"
    ],
    "skipped": [
      {
        "what": "ssh/keys",
        "reason": "not selected"
      }
    ],
    "apps_to_install": [
      "Git"
    ],
    "rollback_available": true
  },
  "error": null
}"#;

/// `session.json` of a LAN send that failed.
const LAN_RECORD: &str = r#"{
  "id": "20261108-101500-d4e5f6",
  "role": "export",
  "state": "failed",
  "created_at": 1,
  "updated_at": 2,
  "source": {
    "kind": "lan",
    "peer_cert_hash": "0707070707070707070707070707070707070707070707070707070707070707",
    "manifest_fingerprint": "0909090909090909090909090909090909090909090909090909090909090909"
  },
  "approved": false,
  "checklist": null,
  "error": "the other machine failed: x"
}"#;

#[test]
fn session_records_round_trip_byte_identical() {
    for golden in [ARCHIVE_RECORD, LAN_RECORD] {
        let record: SessionRecord = serde_json::from_str(golden).unwrap();
        assert_eq!(serde_json::to_string_pretty(&record).unwrap(), golden);
    }
}

// ------------------------------------------------------------------ LAN wire

fn inventory() -> Inventory {
    Inventory {
        os: OsKind::Linux,
        carrova_version: "0.1.0".into(),
        apps: vec![InventoryApp {
            id: "git".into(),
            version: None,
        }],
        free_space: Some(1024),
        device_name: None,
    }
}

fn plan_summary() -> PlanSummary {
    PlanSummary {
        items: vec![ItemPlanSummary {
            item: "git/config".into(),
            counts: ActionCounts {
                create: 1,
                replace: 2,
                keep_both: 3,
                skip: 4,
                identical: 5,
            },
            conflicts: vec![ConflictTarget {
                root: SemanticRoot::UserConfig,
                rel: "git/config".into(),
            }],
        }],
    }
}

#[test]
fn lan_messages_encode_to_golden_cbor() {
    let cases: [(LanMsg, &str); 10] = [
        (
            LanMsg::Inventory(inventory()),
            "a169696e76656e746f7279a4626f73656c696e75786f636172726f76615f76657273696f6e65302e312e30\
             646170707381a2626964636769746776657273696f6ef66a667265655f7370616365190400",
        ),
        (
            LanMsg::Manifest {
                total_len: 3,
                part: vec![1, 2, 3],
            },
            "a1686d616e6966657374a269746f74616c5f6c656e03647061727443010203",
        ),
        (
            LanMsg::PlanSummary(plan_summary()),
            "a16c706c616e5f73756d6d617279a1656974656d7381a3646974656d6a6769742f636f6e66696766636f\
             756e7473a56663726561746501677265706c61636502696b6565705f626f74680364736b697004696964\
             656e746963616c0569636f6e666c6963747381a264726f6f746b757365725f636f6e6669676372656c6a\
             6769742f636f6e666967",
        ),
        (
            LanMsg::Resume {
                stage: ReceiverStage::Planning,
            },
            "a166726573756d65a165737461676568706c616e6e696e67",
        ),
        (
            LanMsg::Resume {
                stage: ReceiverStage::Transferring,
            },
            "a166726573756d65a16573746167656c7472616e7366657272696e67",
        ),
        (LanMsg::Fetch, "656665746368"),
        (
            LanMsg::Applied(checklist()),
            "a1676170706c696564a4656d6f766564816a6769742f636f6e66696767736b697070656481a264776861\
             74687373682f6b65797366726561736f6e6c6e6f742073656c65637465646f617070735f746f5f696e73\
             74616c6c816347697472726f6c6c6261636b5f617661696c61626c65f5",
        ),
        (
            LanMsg::Abort {
                reason: AbortReason::Cancelled,
            },
            "a16561626f7274a166726561736f6ea1646b696e646963616e63656c6c6564",
        ),
        (
            LanMsg::Abort {
                reason: AbortReason::DryRun,
            },
            "a16561626f7274a166726561736f6ea1646b696e64676472795f72756e",
        ),
        (
            LanMsg::Abort {
                reason: AbortReason::Failed {
                    message: "boom".into(),
                },
            },
            "a16561626f7274a166726561736f6ea2646b696e64666661696c6564676d65737361676564626f6f6d",
        ),
    ];
    for (msg, golden) in cases {
        assert_eq!(cbor(&msg), golden, "{msg:?}");
        let mut bytes = Vec::new();
        ciborium::into_writer(&msg, &mut bytes).unwrap();
        let back: LanMsg = ciborium::from_reader(bytes.as_slice()).unwrap();
        assert_eq!(back, msg);
    }
}

#[test]
fn abort_reasons_read_as_before() {
    let texts = [
        (
            AbortReason::Cancelled,
            "the other machine cancelled the session",
        ),
        (
            AbortReason::DryRun,
            "the receiver did a dry run; nothing was transferred",
        ),
        (
            AbortReason::Failed {
                message: "disk\u{202e} on fire\u{7}".into(),
            },
            "the other machine failed: disk on fire",
        ),
    ];
    for (reason, text) in texts {
        assert_eq!(reason.to_string(), text);
    }
}

// ------------------------------------------------------------------ events

#[test]
fn events_serialize_to_golden_json() {
    let session = SessionId("20261108-101500-a1b2c3".into());
    let cases: Vec<(Event, &str)> = vec![
        (
            Event::StateChanged {
                session,
                from: State::Planned,
                to: State::Transferring,
            },
            r#"{"event":"state_changed","session":"20261108-101500-a1b2c3","from":"planned","to":"transferring"}"#,
        ),
        (
            Event::ItemCaptured {
                item: "git/config".into(),
                files: 2,
                bytes: 30,
            },
            r#"{"event":"item_captured","item":"git/config","files":2,"bytes":30}"#,
        ),
        (
            Event::Recaptured {
                path: "user_config:git/config".into(),
                attempt: 1,
            },
            r#"{"event":"recaptured","path":"user_config:git/config","attempt":1}"#,
        ),
        (
            Event::ConflictFound {
                item: "git/config".into(),
                target: "user_config:git/config".into(),
            },
            r#"{"event":"conflict_found","item":"git/config","target":"user_config:git/config"}"#,
        ),
        (
            Event::ActionApplied {
                item: "git/config".into(),
                target: "~/.config/git/config".into(),
                action: "replace".into(),
            },
            r#"{"event":"action_applied","item":"git/config","target":"~/.config/git/config","action":"replace"}"#,
        ),
        (
            Event::RollbackStep {
                target: "~/.config/git/config".into(),
                outcome: RollbackOutcome::MovedAside,
            },
            r#"{"event":"rollback_step","target":"~/.config/git/config","outcome":"moved_aside"}"#,
        ),
        (
            Event::Warning {
                message: "careful".into(),
            },
            r#"{"event":"warning","message":"careful"}"#,
        ),
        (
            Event::Finished {
                checklist: checklist(),
            },
            r#"{"event":"finished","checklist":{"moved":["git/config"],"skipped":[{"what":"ssh/keys","reason":"not selected"}],"apps_to_install":["Git"],"rollback_available":true}}"#,
        ),
        (
            Event::PeerConnected {
                peer: "192.168.1.2:4000".parse().unwrap(),
            },
            r#"{"event":"peer_connected","peer":"192.168.1.2:4000"}"#,
        ),
        (
            Event::PeerLost { retrying: true },
            r#"{"event":"peer_lost","retrying":true}"#,
        ),
        (
            Event::InventoryReceived(inventory()),
            r#"{"event":"inventory_received","os":"linux","carrova_version":"0.1.0","apps":[{"id":"git","version":null}],"free_space":1024}"#,
        ),
        (
            Event::PlanSummaryReceived(plan_summary()),
            r#"{"event":"plan_summary_received","items":[{"item":"git/config","counts":{"create":1,"replace":2,"keep_both":3,"skip":4,"identical":5},"conflicts":[{"root":"user_config","rel":"git/config"}]}]}"#,
        ),
        (
            Event::TransferPaused {
                reason: PauseReason::LowDiskSpace {
                    fs: "/state".into(),
                    available: 1,
                    needed: 2,
                },
            },
            r#"{"event":"transfer_paused","reason":{"kind":"low_disk_space","fs":"/state","available":1,"needed":2}}"#,
        ),
        (Event::TransferResumed, r#"{"event":"transfer_resumed"}"#),
        (Event::PairingFailed, r#"{"event":"pairing_failed"}"#),
    ];
    for (event, golden) in cases {
        assert_eq!(serde_json::to_string(&event).unwrap(), golden);
    }
}

#[test]
fn progress_events_name_their_phase() {
    for (phase, name) in [
        (Phase::Capture, "capture"),
        (Phase::Archive, "archive"),
        (Phase::Send, "send"),
        (Phase::Transfer, "transfer"),
        (Phase::Stage, "stage"),
        (Phase::Apply, "apply"),
    ] {
        let event = Event::Progress {
            phase,
            done: 1,
            total: 2,
        };
        assert_eq!(
            serde_json::to_string(&event).unwrap(),
            format!(r#"{{"event":"progress","phase":"{name}","done":1,"total":2}}"#)
        );
    }
}

// ------------------------------------------------------------------ error texts

/// Texts of the typed session-store errors, as they read when they were
/// all one untyped string variant (front-ends and records show them).
#[test]
fn session_store_errors_read_as_before() {
    let id = SessionId("20261108-101500-a1b2c3".into());
    let session = |problem| CoreError::Session {
        id: id.clone(),
        problem,
    };
    let json_err = serde_json::from_str::<u8>("x").unwrap_err();
    let json_text = json_err.to_string();
    let cbor_err = ciborium::from_reader::<u8, _>(&[0xff][..]).unwrap_err();
    let cbor_text = cbor_err.to_string();
    let cases: Vec<(CoreError, String)> = vec![
        (
            session(SessionProblem::ExportNotResumable),
            format!("session store: session `{id}` is an export; exports are not resumable, run the export again"),
        ),
        (
            session(SessionProblem::ExportNotRollbackable),
            format!("session store: session `{id}` is an export; only imports can be rolled back"),
        ),
        (
            session(SessionProblem::SendNotReceive),
            format!("session store: session `{id}` is a send; resume it as a send"),
        ),
        (
            session(SessionProblem::NotArchiveImport),
            format!("session store: session `{id}` is not an archive import"),
        ),
        (
            session(SessionProblem::NotLanImport),
            format!("session store: session `{id}` is not a LAN import"),
        ),
        (
            session(SessionProblem::NotLanSend),
            format!("session store: session `{id}` is not a LAN send"),
        ),
        (
            session(SessionProblem::NotInterruptedSend),
            format!("session store: session `{id}` is not an interrupted send"),
        ),
        (
            session(SessionProblem::NeverPaired),
            format!("session store: session `{id}` never paired with a receiver; start a new send"),
        ),
        (
            session(SessionProblem::NoResumeSecret),
            format!("session store: session `{id}` has no resume secret (dismissed or ended); start a new receive"),
        ),
        (
            session(SessionProblem::Applying),
            format!("session store: session `{id}` is applying; roll it back or resume it first"),
        ),
        (
            session(SessionProblem::Ended(State::RolledBack)),
            format!("session store: session `{id}` is RolledBack; nothing to resume"),
        ),
        (
            CoreError::RecordMismatch { id: id.clone() },
            format!("session store: session `{id}`: record id does not match"),
        ),
        (
            CoreError::NoSuchSession(id.clone()),
            format!("session store: no session `{id}`"),
        ),
        (
            CoreError::InvalidSessionId("../x".into()),
            "session store: invalid session id `../x`".into(),
        ),
        (
            CoreError::NothingToRollBack,
            "session store: no finished import to roll back".into(),
        ),
        (
            CoreError::NotADirectory("/s/sessions".into()),
            "session store: /s/sessions exists and is not a directory".into(),
        ),
        (
            CoreError::FileTooLarge("/s/plan.json".into()),
            "session store: /s/plan.json is too large".into(),
        ),
        (
            CoreError::MissingFile("manifest.cbor"),
            "session store: session has no manifest.cbor".into(),
        ),
        (
            CoreError::CorruptFile {
                file: format!("{id}/plan.json"),
                reason: Corruption::Json(json_err),
            },
            format!("session store: {id}/plan.json is corrupt: {json_text}"),
        ),
        (
            CoreError::CorruptFile {
                file: "capture.cbor".into(),
                reason: Corruption::Version(9),
            },
            "session store: capture.cbor is corrupt: unsupported version 9".into(),
        ),
        (
            CoreError::CorruptFile {
                file: "capture.cbor".into(),
                reason: Corruption::Mismatch("manifest.cbor"),
            },
            "session store: capture.cbor is corrupt: does not belong to manifest.cbor".into(),
        ),
        (
            CoreError::CorruptFile {
                file: "capture.cbor".into(),
                reason: Corruption::Dangling,
            },
            "session store: capture.cbor is corrupt: dangling chunk location".into(),
        ),
        (
            CoreError::ResumeKey(ResumeKeyProblem::WrongSize),
            "session store: resume.key has the wrong size".into(),
        ),
        (
            CoreError::ResumeKey(ResumeKeyProblem::UnknownMode),
            "session store: resume.key has unknown permissions; refusing to use it (dismiss the session and pair again)".into(),
        ),
        (
            CoreError::ResumeKey(ResumeKeyProblem::Mode {
                found: 0o100_644,
                required: 0o600,
            }),
            "session store: resume.key has mode 100644, not 600; refusing to use it (dismiss the session and pair again)".into(),
        ),
        (
            CoreError::ResumeKey(ResumeKeyProblem::Owner { owner: 0, me: 1000 }),
            "session store: resume.key is owned by user 0, not by you (1000); refusing to use it (dismiss the session and pair again)".into(),
        ),
        (
            CoreError::Random,
            "session store: random number generator failed".into(),
        ),
        (
            CoreError::IdsExhausted,
            "session store: could not allocate a session id".into(),
        ),
        (
            CoreError::LanEnded,
            "session store: the LAN session with the sender has ended".into(),
        ),
        (
            CoreError::FetchAborted,
            "session store: chunk fetch aborted".into(),
        ),
        (
            CoreError::Journal(JournalProblem::Missing),
            "session store: no rollback point for this session".into(),
        ),
        (
            CoreError::Journal(JournalProblem::UnsupportedVersion(99)),
            "session store: unsupported journal version 99".into(),
        ),
        (
            CoreError::Journal(JournalProblem::RecordTooLarge),
            "session store: journal record too large".into(),
        ),
        (
            CoreError::Journal(JournalProblem::FrameLengthCorrupt),
            "session store: journal frame length corrupt".into(),
        ),
        (
            CoreError::Journal(JournalProblem::RecordCorrupt(cbor_err)),
            format!("session store: journal record corrupt: {cbor_text}"),
        ),
        (
            CoreError::InvalidPlanSession,
            "session store: invalid session id in plan".into(),
        ),
    ];
    for (err, text) in cases {
        assert_eq!(err.to_string(), text);
    }
}
