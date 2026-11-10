use super::*;

#[test]
fn phase2_archive_record_loads_as_archive_source() {
    let json = r#"{
            "id": "20261108-101500-a1b2c3",
            "role": "import",
            "state": "planned",
            "created_at": 1793000000,
            "updated_at": 1793000100,
            "archive": "/home/u/backup.carrova",
            "approved": true,
            "checklist": null,
            "error": null,
            "archive_fingerprint": {
                "header": "0101010101010101010101010101010101010101010101010101010101010101",
                "manifest": "0202020202020202020202020202020202020202020202020202020202020202"
            }
        }"#;
    let r: SessionRecord = serde_json::from_str(json).unwrap();
    assert_eq!(r.archive_path(), Some(Path::new("/home/u/backup.carrova")));
    let fp = r.archive_fingerprint().unwrap();
    assert_eq!(fp.header, Blake3Hash([1; 32]));
    assert_eq!(fp.manifest, Blake3Hash([2; 32]));
    assert!(r.approved);
}

#[test]
fn phase2_export_record_without_fingerprint_loads() {
    let json = r#"{"id":"20261108-101500-a1b2c3","role":"export","state":"done",
            "created_at":1,"updated_at":2,"archive":"/tmp/a.carrova","approved":false,
            "checklist":null,"error":null}"#;
    let r: SessionRecord = serde_json::from_str(json).unwrap();
    assert_eq!(
        r.source,
        Some(Source::Archive {
            path: "/tmp/a.carrova".into(),
            fingerprint: None
        })
    );
}

#[test]
fn record_without_source_loads_as_none() {
    let json = r#"{"id":"20261108-101500-a1b2c3","role":"export","state":"scanned",
        "created_at":1,"updated_at":2,"archive":null,"approved":false,
        "checklist":null,"error":null}"#;
    let r: SessionRecord = serde_json::from_str(json).unwrap();
    assert_eq!(r.source, None);
}

#[test]
fn lan_source_round_trips() {
    let r = SessionRecord {
        id: SessionId("20261108-101500-a1b2c3".into()),
        role: Role::Import,
        state: State::Transferring,
        created_at: 1,
        updated_at: 2,
        source: Some(Source::Lan {
            peer_cert_hash: Some(CertHash::from_bytes([7; 32])),
            manifest_fingerprint: Blake3Hash([9; 32]),
            peer_name: None,
        }),
        approved: true,
        checklist: None,
        error: None,
        bytes_moved: None,
        peer_os: None,
        expires_at: None,
    };
    let json = serde_json::to_string(&r).unwrap();
    assert!(!json.contains("\"archive\""), "{json}");
    assert_eq!(serde_json::from_str::<SessionRecord>(&json).unwrap(), r);
}
