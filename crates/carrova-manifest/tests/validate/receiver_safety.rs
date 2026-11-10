//! Receiver-safety regressions: cross-entry checks, mode bits, decode limits.
//!
//! Every malicious manifest is checked both via [`Manifest::validate`] and by
//! raw-encoding it (bypassing `to_cbor`'s validation) and decoding, to prove
//! the receiver's decode path rejects it.

#![allow(clippy::unwrap_used, clippy::panic)]

use std::path::{Path, PathBuf};

use carrova_manifest::{
    DEFAULT_CHUNK_PARAMS, Entry, EntryKind, ItemFlags, Manifest, ManifestError, ManifestItem,
    RelPath, SourceInfo, entry_from_path,
};
use carrova_platform::fake::FakePlatform;
use carrova_platform::{
    FileMeta, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot, SnapshotCapability,
};

fn entry(root: SemanticRoot, rel: &str, kind: EntryKind) -> Entry {
    Entry {
        root,
        rel: RelPath::new(rel).unwrap(),
        kind,
        mode: None,
        mtime_ns: None,
    }
}

fn file(root: SemanticRoot, rel: &str, data: &[u8]) -> Entry {
    let fc = carrova_manifest::chunk_reader(data, DEFAULT_CHUNK_PARAMS).unwrap();
    entry(
        root,
        rel,
        EntryKind::File {
            size: fc.size,
            hash: fc.file_hash,
            chunks: fc.chunks,
        },
    )
}

fn dir(root: SemanticRoot, rel: &str) -> Entry {
    entry(root, rel, EntryKind::Dir)
}

fn link(root: SemanticRoot, rel: &str, target: &str) -> Entry {
    entry(
        root,
        rel,
        EntryKind::Symlink {
            target: target.into(),
        },
    )
}

fn item(id: &str, entries: Vec<Entry>) -> ManifestItem {
    ManifestItem {
        recipe_id: None,
        item_id: id.into(),
        label: id.into(),
        flags: ItemFlags::default(),
        entries,
        handler: None,
        artifacts: Vec::new(),
        source_paths: Vec::new(),
    }
}

fn manifest(items: Vec<ManifestItem>) -> Manifest {
    Manifest::new(
        1_700_000_000,
        SourceInfo {
            os: OsKind::Linux,
            carrova_version: "0.0.1".into(),
            device_name: None,
        },
        items,
    )
}

/// Encode without validation (as an attacker would), then decode.
fn raw_round_trip(m: &Manifest) -> Result<Manifest, ManifestError> {
    let mut buf = Vec::new();
    ciborium::into_writer(m, &mut buf).unwrap();
    Manifest::from_cbor(buf.as_slice())
}

/// Assert both `validate` and decode reject `m` with an error matching `pred`.
fn assert_rejected(m: &Manifest, pred: impl Fn(&ManifestError) -> bool) {
    let v = m.validate().unwrap_err();
    assert!(pred(&v), "validate: {v:?}");
    let d = raw_round_trip(m).unwrap_err();
    assert!(pred(&d), "decode: {d:?}");
    assert!(m.to_cbor_vec().is_err(), "to_cbor must refuse too");
}

fn assert_accepted(m: &Manifest) {
    m.validate().unwrap();
    assert_eq!(&raw_round_trip(m).unwrap(), m);
}

// ---- finding 2a: duplicate (root, rel) ----

fn is_dup(e: &ManifestError, root: SemanticRoot, path: &str) -> bool {
    matches!(e, ManifestError::DuplicateEntry { root: r, path: p } if *r == root && p == path)
}

#[test]
fn duplicate_entry_in_one_item_rejected() {
    let m = manifest(vec![item(
        "a",
        vec![
            file(SemanticRoot::UserConfig, "app/conf", b"one"),
            file(SemanticRoot::UserConfig, "app/conf", b"two"),
        ],
    )]);
    assert_rejected(&m, |e| is_dup(e, SemanticRoot::UserConfig, "app/conf"));
}

#[test]
fn duplicate_entry_across_items_and_kinds_rejected() {
    let m = manifest(vec![
        item("a", vec![dir(SemanticRoot::Home, ".app")]),
        item("b", vec![link(SemanticRoot::Home, ".app", "/etc")]),
    ]);
    assert_rejected(&m, |e| is_dup(e, SemanticRoot::Home, ".app"));
}

#[test]
fn same_rel_under_different_roots_accepted() {
    let m = manifest(vec![item(
        "a",
        vec![
            file(SemanticRoot::UserConfig, "app/conf", b"one"),
            file(SemanticRoot::UserData, "app/conf", b"two"),
        ],
    )]);
    assert_accepted(&m);
}

// ---- finding 2b: write through a symlink / file ancestor ----

fn is_ancestor(e: &ManifestError, path: &str, ancestor: &str) -> bool {
    matches!(
        e,
        ManifestError::NonDirAncestor { root: SemanticRoot::Home, path: p, ancestor: a }
            if p == path && a == ancestor
    )
}

#[test]
fn write_through_symlink_ancestor_rejected() {
    let m = manifest(vec![item(
        "evil",
        vec![
            link(SemanticRoot::Home, ".config/x", "/etc"),
            file(
                SemanticRoot::Home,
                ".config/x/passwd",
                b"root::0:0::/:/bin/sh\n",
            ),
        ],
    )]);
    assert_rejected(&m, |e| is_ancestor(e, ".config/x/passwd", ".config/x"));
}

#[test]
fn symlink_ancestor_rejected_regardless_of_order_item_or_depth() {
    let m = manifest(vec![
        item(
            "a",
            vec![dir(SemanticRoot::Home, ".config/x/a/b")], // descendant first
        ),
        item("b", vec![link(SemanticRoot::Home, ".config", "/etc")]),
    ]);
    assert_rejected(&m, |e| is_ancestor(e, ".config/x/a/b", ".config"));
}

#[test]
fn file_ancestor_rejected() {
    let m = manifest(vec![item(
        "a",
        vec![
            file(SemanticRoot::Home, "a", b"x"),
            file(SemanticRoot::Home, "a/b", b"y"),
        ],
    )]);
    assert_rejected(&m, |e| is_ancestor(e, "a/b", "a"));
}

#[test]
fn dir_ancestors_prefix_siblings_and_other_roots_accepted() {
    let m = manifest(vec![item(
        "a",
        vec![
            dir(SemanticRoot::Home, ".config"),
            dir(SemanticRoot::Home, ".config/x"),
            file(SemanticRoot::Home, ".config/x/conf", b"x"),
            // `x` is a symlink only under user_config, and `xy` is not below `x`.
            link(SemanticRoot::UserConfig, "x", "../elsewhere"),
            file(SemanticRoot::UserConfig, "xy/conf", b"y"),
            file(SemanticRoot::UserData, "x/conf", b"z"),
        ],
    )]);
    assert_accepted(&m);
}

// ---- finding 6: setuid / setgid / sticky ----

fn with_mode(mode: u32) -> Manifest {
    let mut e = file(SemanticRoot::UserData, "app/bin", b"#!/bin/sh\n");
    e.mode = Some(mode);
    manifest(vec![item("a", vec![e])])
}

#[test]
fn special_mode_bits_rejected() {
    for mode in [0o4755, 0o2755, 0o1777, 0o6755, 0o7777, 0o4000] {
        assert_rejected(
            &with_mode(mode),
            |e| matches!(e, ManifestError::UnsafeMode { mode: m, .. } if *m == mode),
        );
    }
}

#[test]
fn ordinary_modes_accepted() {
    for mode in [0o600, 0o700, 0o644, 0o755, 0o400, 0o777, 0o000] {
        assert_accepted(&with_mode(mode));
    }
}

/// Delegates to [`FakePlatform`] but reports setuid+setgid+sticky bits.
#[derive(Debug)]
struct SpecialBitsPlatform(FakePlatform);

impl Platform for SpecialBitsPlatform {
    fn os(&self) -> OsKind {
        self.0.os()
    }
    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        self.0.resolve_root(root)
    }
    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        self.0.find_binary(name)
    }
    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        let mut m = self.0.metadata(path)?;
        m.mode = Some(0o7755);
        Ok(m)
    }
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        self.0.running_processes()
    }
    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        self.0.free_space(path)
    }
    fn snapshot_capability(&self, path: &Path) -> Result<SnapshotCapability, PlatformError> {
        self.0.snapshot_capability(path)
    }
    fn open_read_nofollow(&self, path: &Path) -> Result<std::fs::File, PlatformError> {
        self.0.open_read_nofollow(path)
    }
}

#[test]
fn capture_masks_special_mode_bits() {
    let home = tempfile::tempdir().unwrap();
    let platform = SpecialBitsPlatform(FakePlatform::with_home(home.path()));
    std::fs::write(home.path().join("tool"), b"x").unwrap();
    let e = entry_from_path(&platform, SemanticRoot::Home, "tool", DEFAULT_CHUNK_PARAMS).unwrap();
    assert_eq!(e.mode, Some(0o755));
    assert_accepted(&manifest(vec![item("a", vec![e])]));
}

// ---- finding 8: decode size / nesting limits ----

#[test]
fn oversized_manifest_rejected() {
    let m = manifest(vec![item(
        "a",
        vec![file(SemanticRoot::Home, "a", b"some data")],
    )]);
    let bytes = m.to_cbor_vec().unwrap();
    let len = bytes.len() as u64;

    // Exactly at the limit decodes.
    assert_eq!(
        Manifest::from_cbor_limited(bytes.as_slice(), len).unwrap(),
        m
    );

    let err = Manifest::from_cbor_limited(bytes.as_slice(), len - 1).unwrap_err();
    assert!(
        matches!(err, ManifestError::TooLarge { limit } if limit == len - 1),
        "{err:?}"
    );

    // Huge declared-length string with no data behind it: capped, no OOM.
    let mut evil = vec![0xa1, 0x6e];
    evil.extend_from_slice(b"format_version");
    evil.push(0x7b); // text, 8-byte length follows
    evil.extend_from_slice(&u64::MAX.to_be_bytes());
    evil.extend(std::iter::repeat_n(b'a', 4096));
    let err = Manifest::from_cbor_limited(evil.as_slice(), 1024).unwrap_err();
    assert!(
        matches!(err, ManifestError::TooLarge { limit: 1024 }),
        "{err:?}"
    );
}

#[test]
fn deeply_nested_cbor_rejected_without_overflow() {
    // 200 nested arrays: below ciborium's default (256) but far beyond any
    // real manifest, so only an explicit, tighter limit rejects it. Wrapped
    // in an otherwise valid-looking map so the version peek must skip it.
    for depth in [200usize, 100_000] {
        let mut evil = vec![0xa2, 0x6e];
        evil.extend_from_slice(b"format_version");
        evil.push(0x02);
        evil.extend_from_slice(&[0x61, b'x']);
        evil.extend(std::iter::repeat_n(0x81, depth)); // array(1)
        evil.push(0x00);
        let err = Manifest::from_cbor(evil.as_slice()).unwrap_err();
        assert!(
            matches!(err, ManifestError::Decode(ref m) if m.contains("recursion")),
            "depth {depth}: {err:?}"
        );

        // Bare nesting is rejected too (no usable version), without overflow.
        let mut bare = vec![0x81; depth];
        bare.push(0x00);
        assert!(Manifest::from_cbor(bare.as_slice()).is_err());
    }
}
