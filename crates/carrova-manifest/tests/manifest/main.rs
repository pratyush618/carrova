//! Integration tests: CBOR wire format, decode validation, entries from disk.

#![allow(clippy::unwrap_used, clippy::panic)]

use carrova_manifest::{
    Blake3Hash, ChunkRef, DEFAULT_CHUNK_PARAMS, Entry, EntryKind, ItemFlags, Manifest,
    ManifestError, ManifestItem, Portability, RelPath, SourceInfo, entry_from_path,
};
use ciborium::Value;
use std::fs::File;
use std::path::{Path, PathBuf};

use carrova_platform::fake::FakePlatform;
use carrova_platform::{
    FileKind, FileMeta, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot,
    SnapshotCapability,
};

fn file_entry(rel: &str, data: &[u8]) -> Entry {
    let fc = carrova_manifest::chunk_reader(data, DEFAULT_CHUNK_PARAMS).unwrap();
    Entry {
        root: SemanticRoot::UserConfig,
        rel: RelPath::new(rel).unwrap(),
        kind: EntryKind::File {
            size: fc.size,
            hash: fc.file_hash,
            chunks: fc.chunks,
        },
        mode: Some(0o600),
        mtime_ns: Some(1_700_000_000_123_456_789),
    }
}

fn sample() -> Manifest {
    let big: Vec<u8> = (0..200_000u32)
        .map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8)
        .collect();
    Manifest::new(
        1_700_000_000,
        SourceInfo {
            os: "linux".into(),
            carrova_version: "0.0.1".into(),
        },
        vec![
            ManifestItem {
                recipe_id: Some("git".into()),
                item_id: "config".into(),
                label: "Git config".into(),
                flags: ItemFlags::default(),
                handler: None,
                artifacts: Vec::new(),
                entries: vec![
                    Entry {
                        root: SemanticRoot::UserConfig,
                        rel: RelPath::new("git").unwrap(),
                        kind: EntryKind::Dir,
                        mode: Some(0o755),
                        mtime_ns: None,
                    },
                    file_entry("git/config", b"[user]\n\tname = x\n"),
                    file_entry("git/big", &big),
                    file_entry("git/big-copy", &big),
                    file_entry("git/empty", b""),
                ],
            },
            ManifestItem {
                recipe_id: None,
                item_id: "raw:foo".into(),
                label: "foo".into(),
                flags: ItemFlags {
                    sensitive: true,
                    requires_closed: true,
                    portability: Portability::CrossOs,
                },
                handler: None,
                artifacts: Vec::new(),
                entries: vec![Entry {
                    root: SemanticRoot::Home,
                    rel: RelPath::new(".foo/link").unwrap(),
                    kind: EntryKind::Symlink {
                        target: "../elsewhere".into(),
                    },
                    mode: None,
                    mtime_ns: Some(-5),
                }],
            },
        ],
    )
}

/// Encode, apply `edit` to the generic CBOR value, re-encode, decode.
fn decode_edited(edit: impl FnOnce(&mut Value)) -> Result<Manifest, ManifestError> {
    let mut v = Value::serialized(&sample()).unwrap();
    edit(&mut v);
    let mut buf = Vec::new();
    ciborium::into_writer(&v, &mut buf).unwrap();
    Manifest::from_cbor(buf.as_slice())
}

fn field<'a>(v: &'a mut Value, key: &str) -> &'a mut Value {
    v.as_map_mut()
        .unwrap()
        .iter_mut()
        .find(|(k, _)| k.as_text() == Some(key))
        .map(|(_, v)| v)
        .unwrap()
}

fn first_entry(v: &mut Value) -> &mut Value {
    let items = field(v, "items").as_array_mut().unwrap();
    let entries = field(&mut items[0], "entries").as_array_mut().unwrap();
    &mut entries[1]
}

#[test]
fn cbor_round_trip() {
    let m = sample();
    let bytes = m.to_cbor_vec().unwrap();
    assert_eq!(Manifest::from_cbor(bytes.as_slice()).unwrap(), m);
}

#[test]
fn aggregates_and_dedup() {
    let m = sample();
    let config_len = b"[user]\n\tname = x\n".len() as u64;
    assert_eq!(m.total_size(), config_len + 400_000);
    let big_chunks = m.items[0].entries[2].chunks().len();
    assert!(big_chunks >= 1);
    assert_eq!(m.chunk_count(), 1 + 2 * big_chunks);
    assert_eq!(m.unique_chunks().count(), 1 + big_chunks);
    assert_eq!(m.unique_size(), config_len + 200_000);
    // first occurrence wins
    assert!(
        m.unique_chunks()
            .all(|(e, _)| e.rel.as_str() != "git/big-copy")
    );
}

#[test]
fn future_version_rejected_cleanly() {
    let err = decode_edited(|v| {
        *field(v, "format_version") = Value::Integer(3.into());
        // also add a field v2 doesn't know, as a future format would
        v.as_map_mut()
            .unwrap()
            .push((Value::Text("new_thing".into()), Value::Bool(true)));
    })
    .unwrap_err();
    assert!(
        matches!(
            err,
            ManifestError::UnsupportedFormatVersion { found: 3, .. }
        ),
        "{err:?}"
    );
    assert!(err.to_string().contains("version 3"));
}

#[test]
fn missing_version_rejected() {
    let err = decode_edited(|v| {
        v.as_map_mut()
            .unwrap()
            .retain(|(k, _)| k.as_text() != Some("format_version"));
    })
    .unwrap_err();
    assert!(
        matches!(err, ManifestError::MissingFormatVersion),
        "{err:?}"
    );
}

#[test]
fn encoding_unsupported_version_refused() {
    let mut m = sample();
    m.format_version = 9;
    assert!(matches!(
        m.to_cbor_vec(),
        Err(ManifestError::UnsupportedFormatVersion { found: 9, .. })
    ));
}

#[test]
fn bad_rel_paths_rejected_on_decode() {
    for bad in [
        "../etc/passwd",
        "/etc/passwd",
        "a//b",
        "a/./b",
        "./a",
        "",
        "a\0b",
        "a/",
    ] {
        let res = decode_edited(|v| {
            *field(first_entry(v), "rel") = Value::Text(bad.into());
        });
        assert!(
            matches!(res, Err(ManifestError::Decode(ref m)) if m.contains("invalid relative path")),
            "{bad:?}: {res:?}"
        );
    }
}

#[test]
fn inconsistent_chunks_rejected_on_decode() {
    let res = decode_edited(|v| {
        let kind = field(first_entry(v), "kind");
        let file = field(kind, "file");
        *field(file, "size") = Value::Integer(999.into());
    });
    assert!(matches!(res, Err(ManifestError::Invalid(_))), "{res:?}");
}

#[test]
fn unknown_fields_rejected_within_version() {
    let res = decode_edited(|v| {
        field(first_entry(v), "rel"); // sanity: shape as expected
        first_entry(v)
            .as_map_mut()
            .unwrap()
            .push((Value::Text("owner".into()), Value::Integer(0.into())));
    });
    assert!(matches!(res, Err(ManifestError::Decode(_))), "{res:?}");
}

#[test]
fn garbage_is_decode_error() {
    assert!(Manifest::from_cbor(&b"\xff\x00garbage"[..]).is_err());
    assert!(matches!(
        Manifest::from_cbor(&b"\x01"[..]),
        Err(ManifestError::MissingFormatVersion)
    ));
}

#[test]
fn chunk_ref_hash_is_bytes_in_cbor() {
    let c = ChunkRef {
        hash: Blake3Hash::of(b"a"),
        offset: 0,
        len: 1,
    };
    let v = Value::serialized(&c).unwrap();
    let hash = v
        .as_map()
        .unwrap()
        .iter()
        .find(|(k, _)| k.as_text() == Some("hash"))
        .map(|(_, v)| v.clone())
        .unwrap();
    assert_eq!(hash.as_bytes().map(Vec::len), Some(32));
}

#[test]
fn entry_from_path_file_dir_and_bad_rel() {
    let home = tempfile::tempdir().unwrap();
    let platform = FakePlatform::with_home(home.path());
    let dir = home.path().join(".config/app");
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.json");
    std::fs::write(&file, b"{\"a\":1}").unwrap();

    let e = entry_from_path(
        &platform,
        SemanticRoot::UserConfig,
        "app/settings.json",
        DEFAULT_CHUNK_PARAMS,
    )
    .unwrap();
    assert_eq!(e.rel.as_str(), "app/settings.json");
    assert_eq!(e.file_size(), Some(7));
    match &e.kind {
        EntryKind::File { hash, chunks, .. } => {
            assert_eq!(*hash, Blake3Hash::of(b"{\"a\":1}"));
            assert_eq!(chunks.len(), 1);
        }
        other => panic!("expected file, got {other:?}"),
    }
    assert!(e.mtime_ns.is_some());

    let d = entry_from_path(
        &platform,
        SemanticRoot::UserConfig,
        "./app",
        DEFAULT_CHUNK_PARAMS,
    )
    .unwrap();
    assert_eq!(d.kind, EntryKind::Dir);
    assert_eq!(d.rel.as_str(), "app");

    assert!(matches!(
        entry_from_path(
            &platform,
            SemanticRoot::UserConfig,
            "../x",
            DEFAULT_CHUNK_PARAMS
        ),
        Err(ManifestError::InvalidRelPath { .. })
    ));
}

#[test]
fn symlink_recorded_not_followed() {
    let home = tempfile::tempdir().unwrap();
    let platform = FakePlatform::with_home(home.path());
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret");
    std::fs::write(&secret, b"do not read me").unwrap();
    let link = home.path().join("link");
    carrova_platform::fake::symlink(&secret, &link).unwrap();

    let e = entry_from_path(&platform, SemanticRoot::Home, "link", DEFAULT_CHUNK_PARAMS).unwrap();
    assert_eq!(
        e.kind,
        EntryKind::Symlink {
            target: secret.to_str().unwrap().to_owned()
        }
    );
    assert!(e.chunks().is_empty());

    // dangling link is fine too
    std::fs::remove_file(&secret).unwrap();
    let e = entry_from_path(&platform, SemanticRoot::Home, "link", DEFAULT_CHUNK_PARAMS).unwrap();
    assert!(matches!(e.kind, EntryKind::Symlink { .. }));
}

#[test]
fn v1_manifest_without_handler_fields_still_decodes() {
    let m = sample();
    let mut value: Value = ciborium::from_reader(m.to_cbor_vec().unwrap().as_slice()).unwrap();
    *field(&mut value, "format_version") = Value::Integer(1.into());
    for item in field(&mut value, "items").as_array_mut().unwrap() {
        item.as_map_mut()
            .unwrap()
            .retain(|(k, _)| !matches!(k.as_text(), Some("handler" | "artifacts")));
    }
    let mut buf = Vec::new();
    ciborium::into_writer(&value, &mut buf).unwrap();
    let back = Manifest::from_cbor(buf.as_slice()).unwrap();
    assert_eq!(back.format_version, 1);
    assert!(
        back.items
            .iter()
            .all(|i| i.handler.is_none() && i.artifacts.is_empty())
    );
}

// ---- carry-over regressions: entry_from_path trusts nothing it is handed ----

#[test]
fn entry_path_derived_from_root_and_rel() {
    let home = tempfile::tempdir().unwrap();
    let cfg = tempfile::tempdir().unwrap();
    // UserConfig deliberately not under home: the entry must come from it.
    let platform = FakePlatform::with_home(home.path()).root(SemanticRoot::UserConfig, cfg.path());
    std::fs::create_dir_all(cfg.path().join("app")).unwrap();
    std::fs::write(cfg.path().join("app/f"), b"from-root").unwrap();
    std::fs::create_dir_all(home.path().join(".config/app")).unwrap();
    std::fs::write(home.path().join(".config/app/f"), b"decoy").unwrap();

    let e = entry_from_path(
        &platform,
        SemanticRoot::UserConfig,
        "app/f",
        DEFAULT_CHUNK_PARAMS,
    )
    .unwrap();
    assert_eq!(e.root, SemanticRoot::UserConfig);
    match e.kind {
        EntryKind::File { hash, .. } => assert_eq!(hash, Blake3Hash::of(b"from-root")),
        other => panic!("expected file, got {other:?}"),
    }
}

/// Simulates a file swapped for a symlink between lstat and open: metadata
/// claims every symlink is a regular file; other calls hit the real FS.
#[derive(Debug)]
struct SwappedToSymlink(FakePlatform);

impl Platform for SwappedToSymlink {
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
        if m.kind == FileKind::Symlink {
            m.kind = FileKind::File;
        }
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
    fn open_read_nofollow(&self, path: &Path) -> Result<File, PlatformError> {
        self.0.open_read_nofollow(path)
    }
    fn read_link(&self, path: &Path) -> Result<PathBuf, PlatformError> {
        self.0.read_link(path)
    }
}

#[test]
fn symlink_swapped_in_after_lstat_is_not_followed() {
    let home = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    let secret = outside.path().join("secret");
    std::fs::write(&secret, b"do not read me").unwrap();
    carrova_platform::fake::symlink(&secret, home.path().join("f")).unwrap();

    let platform = SwappedToSymlink(FakePlatform::with_home(home.path()));
    let got = entry_from_path(&platform, SemanticRoot::Home, "f", DEFAULT_CHUNK_PARAMS);
    assert!(
        matches!(got, Err(ManifestError::Platform(PlatformError::Io { .. }))),
        "symlink must be refused by the open, got {got:?}"
    );
}

/// Reports a fixed link target, proving symlinks are read via the platform.
#[derive(Debug)]
struct FixedLinkTarget(FakePlatform);

impl Platform for FixedLinkTarget {
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
        self.0.metadata(path)
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
    fn read_link(&self, _path: &Path) -> Result<PathBuf, PlatformError> {
        Ok(PathBuf::from("via-platform"))
    }
}

#[test]
fn symlink_target_read_through_platform() {
    let home = tempfile::tempdir().unwrap();
    carrova_platform::fake::symlink("real-target", home.path().join("l")).unwrap();
    let platform = FixedLinkTarget(FakePlatform::with_home(home.path()));
    let e = entry_from_path(&platform, SemanticRoot::Home, "l", DEFAULT_CHUNK_PARAMS).unwrap();
    assert_eq!(
        e.kind,
        EntryKind::Symlink {
            target: "via-platform".into()
        }
    );
}
