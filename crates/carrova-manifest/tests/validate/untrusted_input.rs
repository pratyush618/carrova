//! Untrusted-input regressions: terminal-spoofing text and resource limits.
//!
//! Every hostile manifest is checked via [`Manifest::validate`] and by
//! raw-encoding it (bypassing `to_cbor`'s validation) and decoding.

#![allow(clippy::unwrap_used, clippy::panic)]

use carrova_manifest::{
    Artifact, Blake3Hash, ChunkHash, ChunkRef, DEFAULT_CHUNK_PARAMS, DEFAULT_MAX_MANIFEST_BYTES,
    Entry, EntryKind, ItemFlags, MAX_ARTIFACTS_PER_ITEM, MAX_CHUNK_LEN, MAX_ENTRIES, MAX_ITEMS,
    Manifest, ManifestError, ManifestItem, RelPath, SourceInfo,
};
use carrova_platform::SemanticRoot;

fn dir(rel: &str) -> Entry {
    Entry {
        root: SemanticRoot::Home,
        rel: RelPath::new(rel).unwrap(),
        kind: EntryKind::Dir,
        mode: None,
        mtime_ns: None,
    }
}

fn link(rel: &str, target: &str) -> Entry {
    Entry {
        kind: EntryKind::Symlink {
            target: target.into(),
        },
        ..dir(rel)
    }
}

fn artifact(name: &str) -> Artifact {
    let fc = carrova_manifest::chunk_reader(&b"key"[..], DEFAULT_CHUNK_PARAMS).unwrap();
    Artifact {
        name: name.into(),
        size: fc.size,
        hash: fc.file_hash,
        chunks: fc.chunks,
    }
}

fn item(id: &str, entries: Vec<Entry>) -> ManifestItem {
    ManifestItem {
        recipe_id: Some("gnupg".into()),
        item_id: id.into(),
        label: "GnuPG keyring".into(),
        flags: ItemFlags::default(),
        entries,
        handler: Some("gpg-keyring".into()),
        artifacts: vec![artifact("pubring.asc")],
    }
}

fn manifest(items: Vec<ManifestItem>) -> Manifest {
    Manifest::new(
        1_700_000_000,
        SourceInfo {
            os: "linux".into(),
            carrova_version: "0.0.1".into(),
        },
        items,
    )
}

fn sample() -> Manifest {
    manifest(vec![item(
        "keys",
        vec![dir(".gnupg"), link(".gnupg/current", "keys-v2")],
    )])
}

fn raw_round_trip(m: &Manifest) -> Result<Manifest, ManifestError> {
    let mut buf = Vec::new();
    ciborium::into_writer(m, &mut buf).unwrap();
    Manifest::from_cbor(buf.as_slice())
}

fn assert_rejected(m: &Manifest, pred: impl Fn(&ManifestError) -> bool) {
    let v = m.validate().unwrap_err();
    assert!(pred(&v), "validate: {v:?}");
    let d = raw_round_trip(m).unwrap_err();
    assert!(pred(&d), "decode: {d:?}");
    assert!(m.to_cbor_vec().is_err(), "to_cbor must refuse too");
}

// ---- H1: control / bidi characters ----

/// Cursor-up + erase-line, carriage return, newline, DEL, C1 CSI, bidi
/// overrides / isolates / marks.
const EVIL: [&str; 12] = [
    "\x1b[1A\x1b[2K",
    "\r",
    "\n",
    "\t",
    "\x7f",
    "\u{9b}2K",
    "\u{85}",
    "\u{202e}",
    "\u{2066}",
    "\u{200e}",
    "\u{200f}",
    "\u{61c}",
];

fn unsafe_raw(e: &ManifestError) -> bool {
    e.to_string()
        .chars()
        .any(|c| c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{61c}'))
}

#[test]
fn baseline_sample_is_valid() {
    let m = sample();
    m.validate().unwrap();
    assert_eq!(raw_round_trip(&m).unwrap(), m);
}

#[test]
fn control_and_bidi_chars_rejected_in_text_fields() {
    type Edit = fn(&mut Manifest, &str);
    let edits: [(&str, Edit); 8] = [
        ("item_id", |m, s| m.items[0].item_id = format!("keys{s}")),
        ("recipe_id", |m, s| {
            m.items[0].recipe_id = Some(format!("gnupg{s}"));
        }),
        ("label", |m, s| {
            m.items[0].label = format!("GnuPG{s}home:.bashrc");
        }),
        ("handler", |m, s| {
            m.items[0].handler = Some(format!("gpg{s}"));
        }),
        ("artifact name", |m, s| {
            m.items[0].artifacts[0].name = format!("pub{s}ring");
        }),
        ("symlink target", |m, s| {
            m.items[0].entries[1].kind = EntryKind::Symlink {
                target: format!("keys{s}v2"),
            };
        }),
        ("source os", |m, s| m.source.os = format!("linux{s}")),
        ("source carrova_version", |m, s| {
            m.source.carrova_version = format!("0.0.1{s}");
        }),
    ];
    for (field, edit) in edits {
        for evil in EVIL {
            let mut m = sample();
            edit(&mut m, evil);
            assert_rejected(&m, |e| {
                matches!(e, ManifestError::UnsafeText { field: f, .. } if *f == field)
                    && !unsafe_raw(e)
            });
        }
    }
}

#[test]
fn control_and_bidi_chars_rejected_in_rel_paths() {
    for evil in EVIL {
        let p = format!("a{evil}b");
        let err = RelPath::new(p.clone()).unwrap_err();
        assert!(
            matches!(err, ManifestError::InvalidRelPath { .. }) && !unsafe_raw(&err),
            "{p:?}: {err:?}"
        );
        assert!(RelPath::try_from(p).is_err());
    }
}

#[test]
fn ordinary_unicode_text_accepted() {
    let mut m = sample();
    m.items[0].label = "Clés GnuPG — 日本語 ✓".into();
    m.items[0].entries[1].kind = EntryKind::Symlink {
        target: "../ключи v2".into(),
    };
    m.items[0].entries.push(dir(".gnupg/données"));
    m.validate().unwrap();
    assert_eq!(raw_round_trip(&m).unwrap(), m);
}

// ---- M2: size and count limits ----

#[test]
fn default_size_limit_is_32_mib() {
    assert_eq!(DEFAULT_MAX_MANIFEST_BYTES, 32 * 1024 * 1024);
}

fn too_many(e: &ManifestError, what: &str, limit: usize) -> bool {
    matches!(e, ManifestError::TooMany { what: w, limit: l, .. } if *w == what && *l == limit)
}

#[test]
fn too_many_items_rejected() {
    let mut proto = item("x", Vec::new());
    proto.handler = None;
    proto.artifacts.clear();
    let items = (0..=MAX_ITEMS)
        .map(|i| ManifestItem {
            item_id: format!("i{i}"),
            ..proto.clone()
        })
        .collect();
    let m = manifest(items);
    let err = m.validate().unwrap_err();
    assert!(too_many(&err, "items", MAX_ITEMS), "{err:?}");
}

#[test]
fn too_many_entries_rejected() {
    let mut it = item("x", Vec::new());
    it.entries = vec![dir("d"); MAX_ENTRIES + 1];
    let err = manifest(vec![it]).validate().unwrap_err();
    assert!(too_many(&err, "entries", MAX_ENTRIES), "{err:?}");
}

#[test]
fn too_many_artifacts_rejected() {
    let mut it = item("x", Vec::new());
    it.artifacts = (0..=MAX_ARTIFACTS_PER_ITEM)
        .map(|i| artifact(&format!("a{i}")))
        .collect();
    let m = manifest(vec![it]);
    assert_rejected(&m, |e| {
        too_many(e, "artifacts in one item", MAX_ARTIFACTS_PER_ITEM)
    });
}

#[test]
fn trailing_bytes_rejected() {
    let mut bytes = sample().to_cbor_vec().unwrap();
    bytes.push(0);
    let err = Manifest::from_cbor(bytes.as_slice()).unwrap_err();
    assert!(matches!(err, ManifestError::TrailingBytes), "{err:?}");
}

// ---- Chunk lengths (a peer's manifest sizes the receiver's buffers) ----

fn file(rel: &str, chunks: Vec<ChunkRef>) -> Entry {
    let size = chunks.iter().map(|c| u64::from(c.len)).sum();
    Entry {
        kind: EntryKind::File {
            size,
            hash: Blake3Hash::of(rel.as_bytes()),
            chunks,
        },
        ..dir(rel)
    }
}

fn chunk(seed: &[u8], offset: u64, len: u32) -> ChunkRef {
    ChunkRef {
        hash: ChunkHash::of(seed),
        offset,
        len,
    }
}

fn invalid(e: &ManifestError, needle: &str) -> bool {
    matches!(e, ManifestError::Invalid(_)) && e.to_string().contains(needle)
}

#[test]
fn chunk_longer_than_the_chunker_maximum_rejected() {
    let m = manifest(vec![item(
        "x",
        vec![file("big", vec![chunk(b"a", 0, MAX_CHUNK_LEN + 1)])],
    )]);
    assert_rejected(&m, |e| invalid(e, "chunk length"));
}

#[test]
fn chunk_of_exactly_the_maximum_accepted() {
    let m = manifest(vec![item(
        "x",
        vec![file("max", vec![chunk(b"a", 0, MAX_CHUNK_LEN)])],
    )]);
    m.validate().unwrap();
    assert_eq!(MAX_CHUNK_LEN, DEFAULT_CHUNK_PARAMS.max);
}

#[test]
fn zero_length_chunk_rejected() {
    let m = manifest(vec![item("x", vec![file("z", vec![chunk(b"a", 0, 0)])])]);
    assert_rejected(&m, |e| invalid(e, "chunk length"));
}

#[test]
fn one_hash_with_two_lengths_rejected_across_items() {
    let m = manifest(vec![
        item("x", vec![file("one", vec![chunk(b"same", 0, 10)])]),
        item("y", vec![file("two", vec![chunk(b"same", 0, 11)])]),
    ]);
    assert_rejected(&m, |e| invalid(e, "different lengths"));
}

#[test]
fn shared_chunk_with_one_length_accepted() {
    let m = manifest(vec![
        item("x", vec![file("one", vec![chunk(b"same", 0, 10)])]),
        item("y", vec![file("two", vec![chunk(b"same", 0, 10)])]),
    ]);
    m.validate().unwrap();
}
