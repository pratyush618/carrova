//! `carrova devices` and `carrova visibility`, end to end through the
//! binary (HOME and Carrova's state dir in a temp dir). Linux only
//! (`XDG_STATE_HOME` places the state dir; returns early elsewhere).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::presence::EVERYONE_FOR;
use carrova_core::time::now_unix;
use carrova_crypto::identity::{DeviceKey, DevicePublic, Fingerprint};
use carrova_crypto::pairing::KnownSecret;
use carrova_platform::OsKind;
use carrova_platform::fake::FakePlatform;
use serde_json::Value;
use tempfile::TempDir;

/// One user: a temp HOME with its own state dir.
struct User {
    tmp: TempDir,
}

impl User {
    fn new() -> Option<Self> {
        if carrova_platform::current().os() != OsKind::Linux {
            return None;
        }
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("home")).unwrap();
        Some(Self { tmp })
    }

    fn state_dir(&self) -> PathBuf {
        self.tmp.path().join("state/carrova")
    }

    /// Remember the device of `seed` as `name`, as a code-paired move would.
    fn remember(&self, seed: u8, name: &str) -> DevicePublic {
        let platform =
            FakePlatform::with_home(self.tmp.path().join("home")).with_state_dir(self.state_dir());
        let public = DeviceKey::from_bytes(&[seed; 32]).public();
        let record = DeviceRecord {
            public,
            name: name.into(),
            os: OsKind::Linux,
            first_seen: 100,
            last_move: None,
            moves: 2,
            mac_hint: None,
        };
        KnownDevices::open(&platform)
            .unwrap()
            .remember(&platform, record, &KnownSecret::from_bytes([seed; 32]))
            .unwrap();
        public
    }

    /// `carrova ARGS` as this user, stdin not a terminal.
    fn carrova(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_carrova"))
            .args(args)
            .env("HOME", self.tmp.path().join("home"))
            .env("XDG_STATE_HOME", self.tmp.path().join("state"))
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_DATA_HOME")
            .env_remove("XDG_CACHE_HOME")
            .env_remove("RUST_LOG")
            .stdin(Stdio::null())
            .output()
            .unwrap()
    }

    /// `carrova ARGS`, which must succeed; its stdout.
    fn ok(&self, args: &[&str]) -> String {
        let out = self.carrova(args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "carrova {args:?} failed: {stderr}");
        String::from_utf8(out.stdout).unwrap()
    }

    /// `carrova ARGS --json`, which must succeed; its stdout as JSON.
    fn json(&self, args: &[&str]) -> Value {
        let mut args = args.to_vec();
        args.push("--json");
        serde_json::from_str(&self.ok(&args)).expect("stdout is pure JSON")
    }

    fn names(&self) -> Vec<String> {
        self.json(&["devices", "list"])
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["name"].as_str().unwrap().to_owned())
            .collect()
    }
}

#[test]
fn devices_list_shows_fingerprints() {
    let Some(user) = User::new() else { return };
    assert_eq!(user.ok(&["devices", "list"]), "No known devices.\n");
    let desk = user.remember(1, "Desk");

    let listed = user.ok(&["devices"]);

    let fingerprint = Fingerprint::of(&desk).to_string();
    assert!(listed.contains(&fingerprint), "{listed}");
    assert!(listed.contains("Desk"), "{listed}");
    assert!(
        !listed.contains(&desk.to_hex()),
        "never the full key: {listed}"
    );
}

#[test]
fn devices_rename_and_forget_by_fingerprint() {
    let Some(user) = User::new() else { return };
    let desk = user.remember(1, "Desk");
    user.remember(2, "Laptop");
    let fingerprint = Fingerprint::of(&desk).to_string();

    user.ok(&["devices", "rename", &fingerprint, "Office PC"]);
    assert_eq!(user.names(), ["Office PC", "Laptop"]);

    user.ok(&["devices", "forget", &fingerprint]);
    assert_eq!(user.names(), ["Laptop"]);
}

#[test]
fn waking_a_device_without_a_hint_says_so() {
    let Some(user) = User::new() else { return };
    let desk = user.remember(1, "Desk");

    let woke = user.carrova(&["devices", "wake", &Fingerprint::of(&desk).to_string()]);

    assert!(!woke.status.success());
    let stderr = String::from_utf8_lossy(&woke.stderr);
    assert!(stderr.contains("no wake hint saved"), "{stderr}");
}

#[test]
fn forgetting_every_device_needs_confirmation() {
    let Some(user) = User::new() else { return };
    user.remember(1, "Desk");

    let refused = user.carrova(&["devices", "forget", "--all"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("--yes"));
    assert_eq!(user.names(), ["Desk"]);

    user.ok(&["devices", "forget", "--all", "--yes"]);
    assert!(user.names().is_empty());
}

#[test]
fn visibility_everyone_sets_deadline() {
    let Some(user) = User::new() else { return };
    assert_eq!(user.json(&["visibility"])["choice"], "known");

    let before = now_unix();
    let set = user.json(&["visibility", "everyone"]);

    assert_eq!(set["choice"], "everyone");
    let until = set["until"].as_u64().unwrap();
    assert!(
        (before + EVERYONE_FOR.as_secs()..=now_unix() + EVERYONE_FOR.as_secs()).contains(&until),
        "{until}"
    );
    assert_eq!(user.json(&["visibility"]), set, "kept for the next command");
    let shown = user.ok(&["visibility"]);
    assert!(shown.contains("everyone until"), "{shown}");
    assert!(
        shown.contains("at once while the Carrova app runs"),
        "{shown}"
    );

    user.ok(&["visibility", "no-one"]);
    assert_eq!(user.json(&["visibility"])["choice"], "no_one");
}
