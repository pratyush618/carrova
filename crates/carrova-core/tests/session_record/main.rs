//! What a session record tells History: the peer's name and the bytes
//! moved, with records and inventories of older Carrova versions still readable.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/archive.rs"]
mod archive_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use archive_support::{applied, archive};
use carrova_core::lan::{Inventory, LanMsg, MAX_DEVICE_NAME};
use carrova_core::{SessionRecord, Source, sessions};
use carrova_platform::OsKind;

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
