//! Sender persistence for `carrova send` resume: `manifest.cbor` +
//! `capture.cbor` round-trip, and the LAN resume key file.
//!
//! One test function writes the fake `gpg` script (ETXTBSY: see
//! `write_tool_script`), so this binary keeps tool-writing tests single.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::fs;
use std::path::Path;

use carrova_core::session::{Role, State};
use carrova_core::store::{
    RESUME_KEY_FILE, Store, read_resume_secret, wipe_resume_secret, write_resume_secret,
};
use carrova_crypto::pairing::ResumeSecret;
use carrova_platform::fake::{FakePlatform, Hooks};
use carrova_platform::{OsKind, Platform};
use carrova_recipe::load_str;
use tempfile::TempDir;

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

// The fake `gpg` is a `#!/bin/sh` script.
#[cfg(unix)]
mod persisted_capture {
    use carrova_core::capture::{CAPTURE_FILE, Capture, MANIFEST_FILE, capture};
    use carrova_core::select::{Preset, Selection};
    use carrova_core::session::NullSink;
    use carrova_core::{Cancel, ScanOptions, scan};
    use carrova_manifest::{ChunkRef, ChunkSource};
    use carrova_platform::fake::write_tool_script;

    use super::*;

    fn write(path: &Path, bytes: &[u8]) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    fn all_chunks(cap: &Capture) -> Vec<ChunkRef> {
        cap.manifest.unique_chunk_refs().copied().collect()
    }

    #[test]
    fn persisted_capture_serves_identical_chunks_after_reload() {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        write(&home.join(".config/app/a.txt"), b"alpha");
        write(&home.join(".config/app/big.bin"), &vec![3u8; 300_000]);
        write(&home.join(".gnupg/pubring.kbx"), b"keys");
        let gpg = write_tool_script(tmp.path(), "gpg", "printf 'exported %s' \"$*\"\n").unwrap();
        let platform = FakePlatform::with_home(&home)
            .binary("app", "/usr/bin/app")
            .binary("gpg", &gpg);
        let recipes = vec![
            load_str(APP, Path::new("app.yaml")).unwrap(),
            load_str(GPG, Path::new("gpg.yaml")).unwrap(),
        ];
        let report = scan(&recipes, &platform, ScanOptions::default()).unwrap();
        let mut sel = Selection::from_scan(&report, Preset::None);
        sel.set_included("app/config", true).unwrap();
        sel.set_included("gnupg/keyring", true).unwrap();
        let cap = capture(&platform, &recipes, &sel, &NullSink, &Cancel::default()).unwrap();
        assert!(cap.failed.is_empty(), "{:?}", cap.failed);
        let keyring = cap
            .manifest
            .items
            .iter()
            .find(|i| i.item_id == "keyring")
            .unwrap();
        assert!(!keyring.artifacts.is_empty(), "handler artifacts (blobs)");

        let session = tmp.path().join("session");
        fs::create_dir(&session).unwrap();
        let sent = cap.persist(&platform, &session).unwrap();
        for f in [MANIFEST_FILE, CAPTURE_FILE] {
            assert_eq!(
                platform.metadata(&session.join(f)).unwrap().mode,
                Some(0o600)
            );
        }
        // No absolute paths are persisted.
        let raw = fs::read(session.join(CAPTURE_FILE)).unwrap();
        let home_bytes = home.to_str().unwrap().as_bytes();
        assert!(!raw.windows(home_bytes.len()).any(|w| w == home_bytes));

        let loaded = Capture::load(&platform, &session).unwrap();
        assert_eq!(loaded.manifest, cap.manifest);
        assert_eq!(loaded.manifest_cbor().unwrap(), sent, "deterministic bytes");
        let chunks = all_chunks(&cap);
        let mut before = cap.chunk_source(&platform);
        let mut after = loaded.chunk_source(&platform);
        for c in &chunks {
            assert_eq!(before.read_chunk(c).unwrap(), after.read_chunk(c).unwrap());
        }

        // A file changed since capture is still caught after reload.
        fs::write(home.join(".config/app/a.txt"), b"ALPHA").unwrap();
        let a = loaded
            .manifest
            .entries()
            .find(|e| e.rel.as_str() == "app/a.txt")
            .unwrap()
            .chunks()[0];
        assert!(after.read_chunk(&a).is_err());

        // A capture file from another manifest is refused.
        fs::write(session.join(MANIFEST_FILE), b"\xa0").unwrap();
        assert!(Capture::load(&platform, &session).is_err());
    }
}

#[test]
fn resume_key_is_private_and_wiped_when_the_session_ends() {
    let tmp = TempDir::new().unwrap();
    let platform = FakePlatform::with_home(tmp.path());
    let store = Store::open(&platform).unwrap();
    let mut rec = store
        .create(&platform, Role::Import, State::Transferring, None)
        .unwrap();
    let dir = store.session_dir(&rec.id).unwrap();
    let secret = ResumeSecret::from_bytes([42; 32]);

    assert!(read_resume_secret(&platform, &dir).unwrap().is_none());
    write_resume_secret(&platform, &dir, &secret).unwrap();
    let key = dir.join(RESUME_KEY_FILE);
    #[cfg(unix)]
    assert_eq!(platform.metadata(&key).unwrap().mode, Some(0o600));
    let back = read_resume_secret(&platform, &dir).unwrap().unwrap();
    assert_eq!(back.expose_bytes(), secret.expose_bytes());
    // Never overwritten in place.
    assert!(write_resume_secret(&platform, &dir, &secret).is_err());

    // Non-terminal saves keep it; a terminal state wipes it.
    store.save(&platform, &rec).unwrap();
    assert!(key.exists());
    rec.state = State::Failed;
    store.save(&platform, &rec).unwrap();
    assert!(!key.exists());
    assert!(read_resume_secret(&platform, &dir).unwrap().is_none());

    // Explicit wipe (dismiss): idempotent; a planted symlink is unlinked,
    // its target untouched.
    write_resume_secret(&platform, &dir, &secret).unwrap();
    wipe_resume_secret(&platform, &dir).unwrap();
    wipe_resume_secret(&platform, &dir).unwrap();
    let outside = tmp.path().join("outside");
    fs::write(&outside, b"keep").unwrap();
    carrova_platform::fake::symlink(&outside, &key).unwrap();
    assert!(read_resume_secret(&platform, &dir).is_err());
    wipe_resume_secret(&platform, &dir).unwrap();
    assert!(fs::symlink_metadata(&key).is_err());
    assert_eq!(fs::read(&outside).unwrap(), b"keep");

    // Wrong size is an error, not a short secret.
    fs::write(&key, [1u8; 5]).unwrap();
    assert!(read_resume_secret(&platform, &dir).is_err());
}

#[test]
#[cfg(unix)] // mode bits
fn resume_key_readable_by_others_is_refused() {
    let tmp = TempDir::new().unwrap();
    let platform = FakePlatform::with_home(tmp.path());
    let store = Store::open(&platform).unwrap();
    let rec = store
        .create(&platform, Role::Import, State::Transferring, None)
        .unwrap();
    let dir = store.session_dir(&rec.id).unwrap();
    write_resume_secret(&platform, &dir, &ResumeSecret::from_bytes([7; 32])).unwrap();
    let key = dir.join(RESUME_KEY_FILE);
    for mode in [0o640, 0o604, 0o700, 0o400] {
        platform.set_mode(&key, mode).unwrap();
        let err = read_resume_secret(&platform, &dir).unwrap_err().to_string();
        assert!(
            err.contains("600") && err.contains(RESUME_KEY_FILE),
            "{err}"
        );
    }
    platform.set_mode(&key, 0o600).unwrap();
    assert!(read_resume_secret(&platform, &dir).unwrap().is_some());
}

#[test]
#[cfg(unix)] // mode bits
fn resume_key_swapped_after_open_is_judged_by_the_opened_file() {
    let tmp = TempDir::new().unwrap();
    let fake = FakePlatform::with_home(tmp.path());
    let store = Store::open(&fake).unwrap();
    let rec = store
        .create(&fake, Role::Import, State::Transferring, None)
        .unwrap();
    let dir = store.session_dir(&rec.id).unwrap();
    let key = dir.join(RESUME_KEY_FILE);
    // The file actually opened is readable by others ...
    write_resume_secret(&fake, &dir, &ResumeSecret::from_bytes([7; 32])).unwrap();
    fake.set_mode(&key, 0o644).unwrap();
    // ... and a private file takes its name right after the open.
    let private = dir.join("private");
    fs::write(&private, [9u8; 32]).unwrap();
    fake.set_mode(&private, 0o600).unwrap();

    let hooked = fake.clone().with_hooks(Hooks {
        on_open: Some(common::swap_after_open(key.clone(), private)),
        ..Hooks::default()
    });
    let err = read_resume_secret(&hooked, &dir).unwrap_err().to_string();
    assert!(err.contains("644"), "{err}");
}

#[test]
#[cfg(unix)] // the mode check exists only on Unix
fn resume_key_with_unknown_mode_is_refused() {
    let tmp = TempDir::new().unwrap();
    let fake = FakePlatform::with_home(tmp.path());
    let store = Store::open(&fake).unwrap();
    let rec = store
        .create(&fake, Role::Import, State::Transferring, None)
        .unwrap();
    let dir = store.session_dir(&rec.id).unwrap();
    write_resume_secret(&fake, &dir, &ResumeSecret::from_bytes([7; 32])).unwrap();

    let modeless = fake.clone().with_hooks(Hooks {
        hide_mode: true,
        ..Hooks::default()
    });
    let err = read_resume_secret(&modeless, &dir).unwrap_err().to_string();
    assert!(err.contains("permissions"), "{err}");
    assert!(read_resume_secret(&fake, &dir).unwrap().is_some());
}

#[test]
fn resume_key_under_acl_permissions_skips_the_mode_and_owner_checks() {
    let tmp = TempDir::new().unwrap();
    let fake = FakePlatform::with_home(tmp.path());
    let store = Store::open(&fake).unwrap();
    let rec = store
        .create(&fake, Role::Import, State::Transferring, None)
        .unwrap();
    let dir = store.session_dir(&rec.id).unwrap();
    write_resume_secret(&fake, &dir, &ResumeSecret::from_bytes([7; 32])).unwrap();
    fake.set_mode(&dir.join(RESUME_KEY_FILE), 0o644).unwrap();

    // Windows: no mode bits, no uid; the profile's ACLs keep it private.
    let acl = fake.clone().with_os(OsKind::Windows).with_hooks(Hooks {
        hide_mode: true,
        uid: Some(u32::MAX),
        ..Hooks::default()
    });
    assert!(read_resume_secret(&acl, &dir).unwrap().is_some());
    #[cfg(unix)]
    assert!(read_resume_secret(&fake, &dir).is_err());
}

#[test]
#[cfg(unix)] // uids
fn resume_key_owned_by_another_user_is_refused() {
    let tmp = TempDir::new().unwrap();
    let fake = FakePlatform::with_home(tmp.path());
    let store = Store::open(&fake).unwrap();
    let rec = store
        .create(&fake, Role::Import, State::Transferring, None)
        .unwrap();
    let dir = store.session_dir(&rec.id).unwrap();
    write_resume_secret(&fake, &dir, &ResumeSecret::from_bytes([7; 32])).unwrap();
    let owner = fake
        .metadata(&dir.join(RESUME_KEY_FILE))
        .unwrap()
        .uid
        .unwrap();

    let someone_else = fake.clone().with_hooks(Hooks {
        uid: Some(owner.wrapping_add(1)),
        ..Hooks::default()
    });
    let err = read_resume_secret(&someone_else, &dir)
        .unwrap_err()
        .to_string();
    assert!(err.contains("owned by user"), "{err}");
    assert!(read_resume_secret(&fake, &dir).unwrap().is_some());
}

#[test]
fn inventory_lists_detected_apps_and_user_data_free_space() {
    use carrova_core::lan::Inventory;
    let tmp = TempDir::new().unwrap();
    let data = tmp.path().join(".local/share");
    fs::create_dir_all(&data).unwrap();
    let platform = FakePlatform::with_home(tmp.path())
        .binary("app", "/usr/bin/app")
        .free_space_bytes(1)
        .filesystem(&data, 3, 777);
    let recipes = vec![
        load_str(APP, Path::new("app.yaml")).unwrap(),
        load_str(GPG, Path::new("gpg.yaml")).unwrap(),
    ];
    let inv = Inventory::collect(&platform, &recipes);
    assert_eq!(inv.os, carrova_platform::OsKind::Linux);
    assert_eq!(inv.carrova_version, env!("CARGO_PKG_VERSION"));
    let ids: Vec<_> = inv
        .apps
        .iter()
        .map(|a| (a.id.as_str(), a.version.clone()))
        .collect();
    assert_eq!(ids, [("app", None)]);
    assert_eq!(inv.free_space, Some(777));

    // user_data root not created yet: its nearest existing ancestor counts.
    fs::remove_dir_all(&data).unwrap();
    assert_eq!(Inventory::collect(&platform, &recipes).free_space, Some(1));
}
