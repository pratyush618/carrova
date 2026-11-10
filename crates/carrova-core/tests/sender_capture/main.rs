//! Sender capture + export: manifest contents, file-change safety, archive.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use carrova_archive::{ArchiveReader, WriteOptions};
use carrova_core::capture::{Capture, MAX_RECAPTURE, capture};
use carrova_core::select::{Preset, Selection};
use carrova_core::session::{Event, NullSink, Phase, VecSink};
use carrova_core::{Cancel, CoreError, Recipe, ScanOptions, scan};
use carrova_crypto::{KdfParams, Passphrase};
use carrova_manifest::{ChunkSource, EntryKind, Manifest};
use carrova_platform::OsKind;
use carrova_platform::fake::{FakePlatform, Hooks, symlink};
use carrova_recipe::load_str;
use tempfile::TempDir;

mod per_os;

const APP: &str = r"
schema: 1
id: app
name: App
category: dev
detect:
  linux:
    - binary: app
items:
  - id: config
    label: Config
    paths:
      - { root: user_config, rel: app }
    flags: { requires_closed: true }
  - id: key
    label: Key
    paths:
      - { root: home, rel: .app-key }
    flags: { sensitive: true }
  - id: locked
    label: Locked
    paths:
      - { root: user_data, rel: app-locked }
    unmovable: machine-bound
  - id: other
    label: Other
    paths:
      - { root: home, rel: .other }
";

const GPG: &str = r"
schema: 2
id: gnupg
name: GnuPG
category: security
detect:
  linux:
    - binary: gpg
items:
  - id: keyring
    label: Public keyring
    handler: gpg-keyring
    paths:
      - { root: home, rel: .gnupg/pubring.kbx }
";

struct Env {
    _tmp: TempDir,
    home: PathBuf,
    platform: FakePlatform,
    recipes: Vec<Recipe>,
}

fn write(path: &Path, bytes: &[u8]) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn env_with(platform: impl FnOnce(FakePlatform) -> FakePlatform) -> Env {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().to_path_buf();
    write(&home.join(".config/app/settings.json"), b"{\"a\":1}");
    write(&home.join(".config/app/cache.db"), &vec![7u8; 100_000]);
    write(&home.join(".config/app/sub/deep.txt"), b"deep");
    write(&home.join(".config/app/sub/deeper/x"), b"x");
    symlink("settings.json", home.join(".config/app/current")).unwrap();
    write(&home.join(".app-key"), b"secret");
    write(&home.join(".local/share/app-locked/state"), b"bound");
    write(&home.join(".other"), b"other");
    write(&home.join(".config/stray/raw.txt"), b"raw");
    write(&home.join(".config/stray/inner/more.txt"), b"more");
    let p = platform(FakePlatform::with_home(&home).binary("app", "/usr/bin/app"));
    let recipes = vec![load_str(APP, Path::new("app.yaml")).unwrap()];
    Env {
        _tmp: tmp,
        home,
        platform: p,
        recipes,
    }
}

fn env() -> Env {
    env_with(|p| p)
}

impl Env {
    fn selection(&self, preset: Preset) -> Selection {
        let report = scan(&self.recipes, &self.platform, ScanOptions::default()).unwrap();
        Selection::from_scan(&report, preset)
    }

    fn only(&self, keys: &[&str]) -> Selection {
        let mut sel = self.selection(Preset::None);
        for k in keys {
            sel.set_included(k, true).unwrap();
        }
        sel
    }

    fn capture(&self, sel: &Selection) -> Capture {
        capture(
            &self.platform,
            &self.recipes,
            sel,
            &NullSink,
            &Cancel::default(),
        )
        .unwrap()
    }
}

fn paths(m: &Manifest) -> Vec<String> {
    m.entries()
        .map(|e| format!("{}:{}", e.root, e.rel))
        .collect()
}

#[test]
fn captures_suggested_items_with_flags_and_source() {
    let env = env();
    let cap = env.capture(&env.selection(Preset::Suggested));
    assert!(cap.failed.is_empty());
    let m = &cap.manifest;
    assert_eq!(m.source.os, OsKind::Linux);
    assert_eq!(m.items.len(), 2);
    assert_eq!(m.items[0].recipe_id.as_deref(), Some("app"));
    assert_eq!(m.items[0].item_id, "config");
    assert!(m.items[0].flags.requires_closed);
    assert_eq!(m.items[1].item_id, "other");
    assert!(paths(m).contains(&"user_config:app/settings.json".to_owned()));
    let big = m
        .entries()
        .find(|e| e.rel.as_str() == "app/cache.db")
        .unwrap();
    assert_eq!(big.file_size(), Some(100_000));
    assert!(!big.chunks().is_empty());
}

#[test]
fn sensitive_item_blocks_until_confirmed() {
    let env = env();
    let mut sel = env.only(&["app/key"]);
    let err = capture(
        &env.platform,
        &env.recipes,
        &sel,
        &NullSink,
        &Cancel::default(),
    )
    .unwrap_err();
    assert!(matches!(err, CoreError::Selection(_)), "{err}");
    sel.confirm_sensitive("app/key").unwrap();
    let cap = env.capture(&sel);
    assert_eq!(paths(&cap.manifest), ["home:.app-key"]);
    assert!(cap.manifest.items[0].flags.sensitive);
}

#[test]
fn unmovable_item_needs_force() {
    let env = env();
    let mut sel = env.only(&["app/locked"]);
    assert!(
        capture(
            &env.platform,
            &env.recipes,
            &sel,
            &NullSink,
            &Cancel::default()
        )
        .is_err()
    );
    sel.force_unmovable("app/locked").unwrap();
    let cap = env.capture(&sel);
    assert_eq!(
        paths(&cap.manifest),
        ["user_data:app-locked", "user_data:app-locked/state"]
    );
}

#[test]
fn path_exclusion_drops_file_and_subtree() {
    let env = env();
    let mut sel = env.only(&["app/config"]);
    sel.set_path_excluded("app/config", "user_config:app/cache.db", true)
        .unwrap();
    sel.set_path_excluded("app/config", "user_config:app/sub", true)
        .unwrap();
    let cap = env.capture(&sel);
    assert_eq!(
        paths(&cap.manifest),
        [
            "user_config:app",
            "user_config:app/current",
            "user_config:app/settings.json",
        ]
    );
}

#[test]
fn raw_dir_is_captured() {
    let env = env();
    let cap = env.capture(&env.only(&["raw:user_config/stray"]));
    let item = &cap.manifest.items[0];
    assert_eq!(item.recipe_id, None);
    assert_eq!(item.item_id, "user_config/stray");
    assert!(!item.flags.sensitive);
    assert_eq!(
        paths(&cap.manifest),
        [
            "user_config:stray",
            "user_config:stray/inner",
            "user_config:stray/inner/more.txt",
            "user_config:stray/raw.txt",
        ]
    );
}

/// `:` is a valid Linux/macOS name character; only recipe-authored paths
/// reject it (drive letters), not folders the user picks from disk.
#[cfg(unix)]
#[test]
fn raw_dir_with_colon_in_its_name_is_captured() {
    let env = env();
    write(&env.home.join(".config/a:b/x.txt"), b"x");
    let cap = env.capture(&env.only(&["raw:user_config/a:b"]));
    assert_eq!(cap.manifest.items[0].item_id, "user_config/a:b");
    assert_eq!(
        paths(&cap.manifest),
        ["user_config:a:b", "user_config:a:b/x.txt"]
    );
}

#[test]
fn symlink_is_captured_not_followed() {
    let env = env();
    let cap = env.capture(&env.only(&["app/config"]));
    let link = cap
        .manifest
        .entries()
        .find(|e| e.rel.as_str() == "app/current")
        .unwrap();
    assert_eq!(
        link.kind,
        EntryKind::Symlink {
            target: "settings.json".into()
        }
    );
}

#[test]
fn manifest_order_is_deterministic() {
    let env = env();
    let sel = env.selection(Preset::Everything);
    let a = env.capture(&sel).manifest;
    let b = env.capture(&sel).manifest;
    assert_eq!(a.items, b.items);
    // Items in key order; entries per item sorted by (root, rel).
    assert_eq!(a.items.len(), 3);
    assert_eq!(a.items[2].item_id, "user_config/stray");
}

#[test]
fn requires_closed_warns_but_captures() {
    let env = env_with(|p| p.process(42, "app"));
    let sink = VecSink::default();
    let cap = capture(
        &env.platform,
        &env.recipes,
        &env.only(&["app/config"]),
        &sink,
        &Cancel::default(),
    )
    .unwrap();
    assert_eq!(cap.manifest.items.len(), 1);
    let events = sink.events();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Warning { message } if message.contains("running")))
    );
    assert!(
        events.iter().any(
            |e| matches!(e, Event::ItemCaptured { item, files: 4, .. } if item == "app/config")
        )
    );
    assert!(events.iter().any(|e| matches!(
        e,
        Event::Progress { phase: Phase::Capture, done, total } if done == total && *total > 0
    )));
}

// ---- file-change safety -------------------------------------------------

/// `env.platform`, reporting a different mtime for the app settings file on
/// each of its first `changes` lstat calls (a file being written to).
fn flaky(env: &Env, changes: u32) -> FakePlatform {
    let target = env.home.join(".config/app/settings.json");
    let calls = Mutex::new(0u32);
    env.platform.clone().with_hooks(Hooks {
        on_metadata: Some(Arc::new(move |path, meta| {
            if path != target {
                return;
            }
            let mut n = calls.lock().unwrap();
            *n += 1;
            if *n <= changes {
                meta.modified = meta
                    .modified
                    .map(|t| t + Duration::from_secs(u64::from(*n)));
            }
        })),
        ..Hooks::default()
    })
}

fn recaptures(events: &[Event]) -> Vec<u32> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::Recaptured { path, attempt } => {
                assert_eq!(path, "user_config:app/settings.json");
                Some(*attempt)
            }
            _ => None,
        })
        .collect()
}

#[test]
fn file_changing_during_capture_is_recaptured() {
    let env = env();
    // Two lstats per read: first read sees two different mtimes, the
    // second read is stable.
    let p = flaky(&env, 2);
    let sink = VecSink::default();
    let cap = capture(
        &p,
        &env.recipes,
        &env.only(&["app/config"]),
        &sink,
        &Cancel::default(),
    )
    .unwrap();
    assert!(cap.failed.is_empty());
    assert_eq!(recaptures(&sink.events()), [1]);
    assert!(paths(&cap.manifest).contains(&"user_config:app/settings.json".to_owned()));
}

#[test]
fn file_that_never_settles_fails_its_item_only() {
    let env = env();
    let p = flaky(&env, u32::MAX);
    let sink = VecSink::default();
    let sel = env.only(&["app/config", "app/other"]);
    let cap = capture(&p, &env.recipes, &sel, &sink, &Cancel::default()).unwrap();
    assert_eq!(
        recaptures(&sink.events()),
        (1..=MAX_RECAPTURE).collect::<Vec<_>>()
    );
    assert_eq!(cap.failed.len(), 1);
    assert_eq!(cap.failed[0].what, "app/config");
    assert!(cap.failed[0].reason.contains("kept changing"));
    assert_eq!(paths(&cap.manifest), ["home:.other"]);
}

#[test]
fn chunk_source_verifies_bytes_still_match() {
    let env = env();
    let cap = env.capture(&env.only(&["app/config"]));
    let entry = |rel: &str| {
        cap.manifest
            .entries()
            .find(|e| e.rel.as_str() == rel)
            .unwrap()
            .chunks()[0]
    };
    let settings = entry("app/settings.json");
    let deep = entry("app/sub/deep.txt");
    let mut src = cap.chunk_source(&env.platform);
    assert_eq!(src.read_chunk(&settings).unwrap(), b"{\"a\":1}");

    // Same length, different content.
    fs::write(env.home.join(".config/app/settings.json"), b"{\"a\":2}").unwrap();
    let err = src.read_chunk(&settings).unwrap_err();
    assert!(err.to_string().contains("changed after capture"), "{err}");
    // Truncated.
    fs::write(env.home.join(".config/app/sub/deep.txt"), b"de").unwrap();
    assert!(src.read_chunk(&deep).is_err());
    // Replaced by a symlink: never followed.
    let deep_path = env.home.join(".config/app/sub/deep.txt");
    fs::remove_file(&deep_path).unwrap();
    symlink("../settings.json", &deep_path).unwrap();
    assert!(src.read_chunk(&deep).is_err());
}

#[test]
fn handler_item_never_captures_raw_files() {
    let mut env = env_with(|p| p.binary("gpg", "/usr/bin/gpg"));
    env.recipes
        .push(load_str(GPG, Path::new("gpg.yaml")).unwrap());
    write(&env.home.join(".gnupg/pubring.kbx"), b"keys");
    let cap = env.capture(&env.only(&["gnupg/keyring", "app/other"]));
    // While handlers are unimplemented (or gpg is unavailable) the item
    // fails on its own; once implemented it carries artifacts, no entries.
    match cap.manifest.items.iter().find(|i| i.item_id == "keyring") {
        None => {
            assert_eq!(cap.failed.len(), 1);
            assert_eq!(cap.failed[0].what, "gnupg/keyring");
        }
        Some(item) => {
            assert_eq!(item.handler.as_deref(), Some("gpg-keyring"));
            assert!(item.entries.is_empty());
        }
    }
    assert_eq!(paths(&cap.manifest), ["home:.other"]);
}

#[test]
fn export_writes_readable_archive() {
    let env = env();
    let cap = env.capture(&env.selection(Preset::Everything));
    let out = env.home.join("out.carrova");
    let pass = Passphrase::new("correct horse".into());
    let sink = VecSink::default();
    let opts = WriteOptions {
        kdf: KdfParams::FLOOR,
    };
    let summary = carrova_core::export::write(
        &env.platform,
        &cap,
        &out,
        &pass,
        opts,
        &sink,
        &Cancel::default(),
    )
    .unwrap();
    assert_eq!(
        summary.chunks,
        u64::try_from(cap.manifest.unique_chunk_refs().count()).unwrap()
    );
    let last = sink
        .events()
        .into_iter()
        .filter_map(|e| match e {
            Event::Progress {
                phase: Phase::Archive,
                done,
                total,
            } => Some((done, total)),
            _ => None,
        })
        .next_back()
        .unwrap();
    assert_eq!(last.0, last.1);

    let mut reader = ArchiveReader::open(&env.platform, &out, &pass).unwrap();
    assert_eq!(reader.manifest(), &cap.manifest);
    let n = reader.chunks().map(Result::unwrap).count();
    assert_eq!(u64::try_from(n).unwrap(), summary.chunks);

    // Existing target is never overwritten.
    assert!(
        carrova_core::export::write(
            &env.platform,
            &cap,
            &out,
            &pass,
            opts,
            &NullSink,
            &Cancel::default()
        )
        .is_err()
    );
}

#[test]
fn export_fails_if_file_changed_after_capture() {
    let env = env();
    let cap = env.capture(&env.only(&["app/config"]));
    fs::write(env.home.join(".config/app/settings.json"), b"{\"a\":9}").unwrap();
    let out = env.home.join("out.carrova");
    let opts = WriteOptions {
        kdf: KdfParams::FLOOR,
    };
    let pass = Passphrase::new("pw".into());
    assert!(
        carrova_core::export::write(
            &env.platform,
            &cap,
            &out,
            &pass,
            opts,
            &NullSink,
            &Cancel::default()
        )
        .is_err()
    );
    assert!(!out.exists());
}

#[test]
#[cfg(unix)] // Windows refuses control characters in file names
fn control_chars_in_names_are_skipped_with_reason_not_fatal() {
    let e = env();
    // Legal on Linux, rejected by the manifest (terminal-escape spoofing).
    write(&e.home.join(".config/app/bad\nname"), b"x");
    write(&e.home.join(".config/app/esc\u{1b}[2Jdir/inner"), b"y");
    symlink("evil\u{1b}target", e.home.join(".config/app/badlink")).unwrap();
    let cap = e.capture(&e.only(&["app/config"]));
    let got = paths(&cap.manifest);
    assert!(got.contains(&"user_config:app/settings.json".to_owned()));
    assert!(
        got.iter()
            .all(|p| !p.contains('\n') && !p.contains('\u{1b}'))
    );
    assert!(!got.contains(&"user_config:app/badlink".to_owned()));
    let reasons: Vec<_> = cap
        .failed
        .iter()
        .map(|s| (s.what.as_str(), s.reason.as_str()))
        .collect();
    assert_eq!(reasons.len(), 3, "{reasons:?}");
    assert!(
        reasons
            .iter()
            .all(|(w, r)| !w.contains('\n') && !w.contains('\u{1b}') && r.contains("control"))
    );
}
