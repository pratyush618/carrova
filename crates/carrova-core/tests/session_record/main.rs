//! What a session record tells History: the peer's name and the bytes
//! moved, with records and inventories of older Carrova versions still readable,
//! and when an undo point expires.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/archive.rs"]
mod archive_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::sync::{Arc, Mutex};

use archive_support::{Side, applied, applied_with, archive, pass};
use carrova_core::lan::{Inventory, LanMsg, MAX_DEVICE_NAME};
use carrova_core::store::Store;
use carrova_core::{
    Engine, Importer, PlanOptions, Retention, SessionId, SessionRecord, Source, State, VecSink,
    sessions, time,
};
use carrova_platform::OsKind;
use carrova_platform::fake::{FakePlatform, Hooks, RenameHook};

/// `session.json` of a finished LAN send that knows its receiver.
const NAMED_LAN_RECORD: &str = r#"{
  "id": "20261108-101500-d4e5f6",
  "role": "export",
  "state": "done",
  "created_at": 1,
  "updated_at": 2,
  "source": {
    "kind": "lan",
    "peer_cert_hash": "0707070707070707070707070707070707070707070707070707070707070707",
    "manifest_fingerprint": "0909090909090909090909090909090909090909090909090909090909090909",
    "peer_name": "ana-laptop"
  },
  "approved": false,
  "checklist": null,
  "error": null,
  "bytes_moved": 4096
}"#;

#[test]
fn peer_name_and_bytes_moved_round_trip_byte_identical() {
    let record: SessionRecord = serde_json::from_str(NAMED_LAN_RECORD).unwrap();
    assert_eq!(record.bytes_moved, Some(4096));
    assert!(matches!(
        &record.source,
        Some(Source::Lan { peer_name: Some(name), .. }) if name == "ana-laptop"
    ));
    assert_eq!(
        serde_json::to_string_pretty(&record).unwrap(),
        NAMED_LAN_RECORD
    );
}

#[test]
fn records_without_the_new_fields_read_as_unknown() {
    let json = r#"{"id":"20261108-101500-a1b2c3","role":"export","state":"done",
        "created_at":1,"updated_at":2,"source":{"kind":"lan",
        "manifest_fingerprint":"0909090909090909090909090909090909090909090909090909090909090909"},
        "approved":false,"checklist":null,"error":null}"#;
    let record: SessionRecord = serde_json::from_str(json).unwrap();
    assert_eq!(record.bytes_moved, None);
    assert!(matches!(
        record.source,
        Some(Source::Lan {
            peer_name: None,
            ..
        })
    ));
}

#[test]
fn finished_archive_sessions_record_the_bytes_moved() {
    let exported = archive();
    assert_eq!(exported.record.bytes_moved, Some(exported.summary.bytes));

    let (r, imp) = applied();
    let written = imp.current_plan().unwrap().bytes_to_write;
    let record = sessions::show(&r.engine, imp.id()).unwrap();
    assert_eq!(record.bytes_moved, Some(written));
    assert!(written > 0);
}

const DAY: u64 = 86_400;

#[test]
fn undo_points_are_kept_until_dismissed_by_default() {
    let (r, imp) = applied();
    assert_eq!(
        sessions::show(&r.engine, imp.id()).unwrap().expires_at,
        None
    );
    assert!(
        sessions::prune_expired(&r.engine, u64::MAX)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn a_kept_for_undo_point_expires_after_its_days_and_is_then_pruned() {
    let before = time::now_unix();
    let (r, imp) = applied_with(&PlanOptions {
        retention: Retention::KeepFor { days: 7 },
        ..PlanOptions::default()
    });
    let expires = sessions::show(&r.engine, imp.id())
        .unwrap()
        .expires_at
        .unwrap();
    assert!((before + 7 * DAY..=time::now_unix() + 7 * DAY).contains(&expires));

    assert!(
        sessions::prune_expired(&r.engine, expires - 1)
            .unwrap()
            .is_empty()
    );
    assert!(sessions::rollback_preview(&r.engine, Some(imp.id())).is_ok());

    let pruned = sessions::prune_expired(&r.engine, expires).unwrap();
    assert_eq!(pruned, [imp.id().clone()]);
    let record = sessions::show(&r.engine, imp.id()).unwrap();
    assert_eq!(record.expires_at, None);
    assert!(!record.checklist.unwrap().rollback_available);
    assert!(!r.session_dir(imp.id()).join("backup").exists());
    assert!(
        sessions::prune_expired(&r.engine, u64::MAX)
            .unwrap()
            .is_empty()
    );
}

/// Plan options keeping the undo point for 7 days.
fn a_week() -> PlanOptions {
    PlanOptions {
        retention: Retention::KeepFor { days: 7 },
        ..PlanOptions::default()
    }
}

/// [`applied_with`] [`a_week`].
fn kept_for_a_week() -> (Side, Importer) {
    applied_with(&a_week())
}

/// Rewrite the persisted record of `id` with `edit` (a record another
/// process left behind).
fn edit_record(r: &Side, id: &SessionId, edit: impl FnOnce(&mut SessionRecord)) {
    let store = Store::open(&*r.platform).unwrap();
    let mut record = store.load(&*r.platform, id).unwrap();
    edit(&mut record);
    store.save(&*r.platform, &record).unwrap();
}

#[test]
fn an_incomplete_rollback_then_expiry_keeps_the_backup() {
    let (r, imp) = kept_for_a_week();
    // Edited after the import: rollback leaves it, and its backup, alone.
    r.put(".config/app/settings.json", b"edited after the import");
    let report = sessions::rollback(&r.engine, Some(imp.id())).unwrap();
    assert!(!report.failed.is_empty(), "{report:?}");

    assert!(
        sessions::prune_expired(&r.engine, u64::MAX)
            .unwrap()
            .is_empty()
    );
    let record = sessions::show(&r.engine, imp.id()).unwrap();
    assert_eq!(record.expires_at, None);
    assert!(record.error.is_some());
    assert!(r.session_dir(imp.id()).join("backup").exists());
}

#[test]
fn a_session_with_an_error_is_never_pruned() {
    let (r, imp) = kept_for_a_week();
    edit_record(&r, imp.id(), |rec| {
        rec.error = Some("rollback incomplete".into());
    });
    assert!(
        sessions::prune_expired(&r.engine, u64::MAX)
            .unwrap()
            .is_empty()
    );
    assert!(r.session_dir(imp.id()).join("backup").exists());
}

#[test]
fn prune_skips_a_session_whose_rollback_is_in_progress() {
    let (r, imp) = kept_for_a_week();
    // Another process prunes as this rollback restores its first file.
    let pruned_mid_rollback = Arc::new(Mutex::new(None));
    let hook: RenameHook = {
        let (other, home, seen) = (
            r.engine.clone(),
            r.home.clone(),
            pruned_mid_rollback.clone(),
        );
        Arc::new(move |_, to| {
            let mut seen = seen.lock().unwrap();
            if seen.is_none() && to.starts_with(&home) {
                *seen = Some(sessions::prune_expired(&other, u64::MAX).unwrap());
            }
            Ok(())
        })
    };
    let hooked = Arc::new(
        FakePlatform::with_home(&r.home)
            .with_state_dir(r.tmp.path().join("state/carrova"))
            .with_hooks(Hooks {
                on_rename: Some(hook),
                ..Hooks::default()
            }),
    );
    let engine = Engine::new(hooked, Vec::new(), Arc::new(VecSink::default()));

    let report = sessions::rollback(&engine, Some(imp.id())).unwrap();
    assert!(report.failed.is_empty(), "{:?}", report.failed);
    assert_eq!(pruned_mid_rollback.lock().unwrap().take(), Some(Vec::new()));
    assert_eq!(
        sessions::show(&r.engine, imp.id()).unwrap().state,
        State::RolledBack
    );
}

#[test]
fn one_failing_session_does_not_stop_pruning_the_others() {
    let (r, first) = kept_for_a_week();
    let mut second = Importer::open(r.engine.clone(), &archive().path, &pass()).unwrap();
    second.plan(&a_week()).unwrap();
    second.approve().unwrap();
    second.apply().unwrap();
    // Looks like an apply still running elsewhere: dismissing it is refused.
    edit_record(&r, first.id(), |rec| rec.state = State::Applying);

    let pruned = sessions::prune_expired(&r.engine, u64::MAX).unwrap();
    assert_eq!(pruned, [second.id().clone()]);
    assert!(r.session_dir(first.id()).join("backup").exists());
    assert!(!r.session_dir(second.id()).join("backup").exists());
}

#[test]
fn an_archive_import_records_the_source_os() {
    let (r, imp) = applied();
    let record = sessions::show(&r.engine, imp.id()).unwrap();
    assert_eq!(record.peer_os, Some(OsKind::Linux));
}

fn inventory(device_name: Option<&str>) -> Inventory {
    Inventory {
        os: OsKind::Linux,
        carrova_version: "0.1.0".into(),
        apps: Vec::new(),
        free_space: None,
        device_name: device_name.map(Into::into),
    }
}

#[test]
fn an_inventory_carries_the_device_name_and_older_ones_still_decode() {
    let msg = LanMsg::Inventory(inventory(Some("ana-laptop")));
    let mut bytes = Vec::new();
    ciborium::into_writer(&msg, &mut bytes).unwrap();
    assert_eq!(
        ciborium::from_reader::<LanMsg, _>(bytes.as_slice()).unwrap(),
        msg
    );

    let old = LanMsg::Inventory(inventory(None));
    let mut old_bytes = Vec::new();
    ciborium::into_writer(&old, &mut old_bytes).unwrap();
    assert!(
        !String::from_utf8_lossy(&old_bytes).contains("device_name"),
        "absent names are not sent"
    );
}

#[test]
fn an_announced_device_name_is_sanitised_before_use() {
    assert_eq!(
        inventory(Some(" ana\u{202e}-laptop\u{7} ")).device_name(),
        Some("ana-laptop".into())
    );
    assert_eq!(inventory(Some("\u{7}  ")).device_name(), None);
    let long = "x".repeat(MAX_DEVICE_NAME * 2);
    assert_eq!(
        inventory(Some(&long))
            .device_name()
            .unwrap()
            .chars()
            .count(),
        MAX_DEVICE_NAME
    );
}
