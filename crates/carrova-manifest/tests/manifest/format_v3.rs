//! Format v3: typed source OS (stored as its stable id) and per-item
//! source paths; v2 manifests still decode.

use carrova_manifest::{
    FORMAT_VERSION, MAX_SOURCE_PATHS_PER_ITEM, Manifest, ManifestError, RelPath, SourcePath,
};
use carrova_platform::{OsKind, SemanticRoot};
use ciborium::Value;

use super::{decode_edited, field, sample};

fn source_path(rel: &str) -> SourcePath {
    SourcePath {
        root: SemanticRoot::UserConfig,
        rel: RelPath::new(rel).unwrap(),
    }
}

fn encoded(m: &Manifest) -> Value {
    ciborium::from_reader(m.to_cbor_vec().unwrap().as_slice()).unwrap()
}

#[test]
fn new_manifests_are_format_3() {
    assert_eq!(FORMAT_VERSION, 3);
    assert_eq!(sample().format_version, 3);
}

#[test]
fn source_os_is_written_as_its_stable_id() {
    let mut m = sample();
    m.source.os = OsKind::MacOs;
    let mut v = encoded(&m);
    let os = field(field(&mut v, "source"), "os");
    assert_eq!(os.as_text(), Some("macos"));
}

#[test]
fn source_paths_round_trip() {
    let mut m = sample();
    m.source.os = OsKind::Windows;
    m.items[0].source_paths = vec![source_path("Code/User"), source_path("Code/keys")];
    let back = Manifest::from_cbor(m.to_cbor_vec().unwrap().as_slice()).unwrap();
    assert_eq!(back, m);
}

#[test]
fn v2_manifest_reads_with_typed_os_and_no_source_paths() {
    for (id, os) in [("linux", OsKind::Linux), ("macos", OsKind::MacOs)] {
        let back = decode_edited(|v| {
            *field(v, "format_version") = Value::Integer(2.into());
            *field(field(v, "source"), "os") = Value::Text(id.into());
            for item in field(v, "items").as_array_mut().unwrap() {
                item.as_map_mut()
                    .unwrap()
                    .retain(|(k, _)| k.as_text() != Some("source_paths"));
            }
        })
        .unwrap();
        assert_eq!(back.format_version, 2);
        assert_eq!(back.source.os, os);
        assert!(back.items.iter().all(|i| i.source_paths.is_empty()));
    }
}

#[test]
fn unknown_source_os_is_a_decode_error() {
    for bad in ["freebsd", "mac_os", "Linux", ""] {
        let err = decode_edited(|v| {
            *field(field(v, "source"), "os") = Value::Text(bad.into());
        })
        .unwrap_err();
        assert!(
            matches!(&err, ManifestError::Decode(msg) if msg.contains("unknown OS")),
            "{bad:?}: {err:?}"
        );
    }
}

#[test]
fn bad_source_path_is_rejected_on_decode() {
    let err = decode_edited(|v| {
        let items = field(v, "items").as_array_mut().unwrap();
        *field(&mut items[0], "source_paths") = Value::Array(vec![Value::Map(vec![
            (Value::Text("root".into()), Value::Text("home".into())),
            (Value::Text("rel".into()), Value::Text("../etc".into())),
        ])]);
    })
    .unwrap_err();
    assert!(
        matches!(&err, ManifestError::Decode(msg) if msg.contains("invalid relative path")),
        "{err:?}"
    );
}

#[test]
fn too_many_source_paths_are_rejected() {
    let mut m = sample();
    m.items[0].source_paths = vec![source_path("x"); MAX_SOURCE_PATHS_PER_ITEM + 1];
    assert!(matches!(
        m.validate(),
        Err(ManifestError::TooMany {
            what: "source paths in one item",
            ..
        })
    ));
}
