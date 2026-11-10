//! Guard: exact `Display` text of every `Manifest::validate` rejection.
//! Users and logs see these strings; refactors must keep them identical.

#![allow(clippy::unwrap_used, clippy::panic)]

use carrova_manifest::{
    Artifact, Blake3Hash, ChunkHash, ChunkRef, Entry, EntryKind, ItemFlags, MAX_CHUNK_LEN,
    Manifest, ManifestItem, RelPath, SourceInfo,
};
use carrova_platform::{OsKind, SemanticRoot};

fn dir(rel: &str) -> Entry {
    Entry {
        root: SemanticRoot::Home,
        rel: RelPath::new(rel).unwrap(),
        kind: EntryKind::Dir,
        mode: None,
        mtime_ns: None,
    }
}

fn chunk(seed: &[u8], offset: u64, len: u32) -> ChunkRef {
    ChunkRef {
        hash: ChunkHash::of(seed),
        offset,
        len,
    }
}

fn file(rel: &str, size: u64, chunks: Vec<ChunkRef>) -> Entry {
    Entry {
        kind: EntryKind::File {
            size,
            hash: Blake3Hash::of(rel.as_bytes()),
            chunks,
        },
        ..dir(rel)
    }
}

fn artifact(name: &str, size: u64, chunks: Vec<ChunkRef>) -> Artifact {
    Artifact {
        name: name.into(),
        size,
        hash: Blake3Hash::of(name.as_bytes()),
        chunks,
    }
}

fn item(id: &str) -> ManifestItem {
    ManifestItem {
        recipe_id: Some("app".into()),
        item_id: id.into(),
        label: "App".into(),
        flags: ItemFlags::default(),
        entries: Vec::new(),
        handler: None,
        artifacts: Vec::new(),
        source_paths: Vec::new(),
    }
}

fn rejection(items: Vec<ManifestItem>) -> String {
    let source = SourceInfo {
        os: OsKind::Linux,
        carrova_version: "0.0.1".into(),
        device_name: None,
    };
    Manifest::new(0, source, items)
        .validate()
        .unwrap_err()
        .to_string()
}

#[test]
fn item_field_rejections_keep_their_text() {
    let empty_id = item("");
    assert_eq!(rejection(vec![empty_id]), "invalid manifest: empty item_id");

    let empty_recipe = ManifestItem {
        recipe_id: Some(String::new()),
        ..item("x")
    };
    assert_eq!(
        rejection(vec![empty_recipe]),
        "invalid manifest: item x: empty recipe_id"
    );

    let empty_handler = ManifestItem {
        handler: Some(String::new()),
        ..item("x")
    };
    assert_eq!(
        rejection(vec![empty_handler]),
        "invalid manifest: item x: empty handler"
    );

    let orphan_artifacts = ManifestItem {
        artifacts: vec![artifact("a", 0, Vec::new())],
        ..item("x")
    };
    assert_eq!(
        rejection(vec![orphan_artifacts]),
        "invalid manifest: item x: artifacts without handler"
    );

    let duplicate_artifact = ManifestItem {
        handler: Some("h".into()),
        artifacts: vec![artifact("a", 0, Vec::new()), artifact("a", 0, Vec::new())],
        ..item("x")
    };
    assert_eq!(
        rejection(vec![duplicate_artifact]),
        "invalid manifest: item x: empty or duplicate artifact name"
    );
}

#[test]
fn chunk_list_rejections_keep_their_text() {
    let artifact_gap = ManifestItem {
        handler: Some("h".into()),
        artifacts: vec![artifact("a", 4, vec![chunk(b"a", 1, 3)])],
        ..item("x")
    };
    assert_eq!(
        rejection(vec![artifact_gap]),
        "invalid manifest: artifact a: chunks not contiguous"
    );

    let cases = [
        (
            file("f", 1, vec![chunk(b"a", 0, 0)]),
            format!("invalid manifest: home/f: chunk length 0 outside 1..={MAX_CHUNK_LEN}"),
        ),
        (
            file("f", 9, vec![chunk(b"a", 0, 3)]),
            "invalid manifest: home/f: chunks do not cover file size".to_owned(),
        ),
        (
            file("f", 0, vec![chunk(b"a", 0, 1), chunk(b"b", u64::MAX, 1)]),
            "invalid manifest: home/f: chunks not contiguous".to_owned(),
        ),
        (
            file("f", 0, vec![chunk(b"a", u64::MAX, 1)]),
            "invalid manifest: home/f: chunks not contiguous".to_owned(),
        ),
        (
            Entry {
                kind: EntryKind::Symlink {
                    target: String::new(),
                },
                ..dir("l")
            },
            "invalid manifest: home/l: empty symlink target".to_owned(),
        ),
    ];
    for (entry, expected) in cases {
        let it = ManifestItem {
            entries: vec![entry],
            ..item("x")
        };
        assert_eq!(rejection(vec![it]), expected);
    }
}

#[test]
fn conflicting_chunk_lengths_keep_their_text() {
    let a = ManifestItem {
        entries: vec![file("one", 10, vec![chunk(b"same", 0, 10)])],
        ..item("x")
    };
    let b = ManifestItem {
        entries: vec![file("two", 11, vec![chunk(b"same", 0, 11)])],
        ..item("y")
    };
    assert_eq!(
        rejection(vec![a, b]),
        format!(
            "invalid manifest: chunk {} listed with different lengths",
            ChunkHash::of(b"same")
        )
    );
}
