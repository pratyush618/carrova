//! One user of the `carrova` binary: a temp HOME with Carrova's state dir
//! in it (`XDG_STATE_HOME`). Linux only ([`User::new`] is `None`
//! elsewhere).

use std::path::PathBuf;
use std::process::{Child, Command, Output, Stdio};

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::KnownSecret;
use carrova_platform::OsKind;
use carrova_platform::fake::FakePlatform;
use serde_json::Value;
use tempfile::TempDir;

/// One user: a temp HOME with its own state dir.
pub struct User {
    tmp: TempDir,
}

impl User {
    pub fn new() -> Option<Self> {
        if carrova_platform::current().os() != OsKind::Linux {
            return None;
        }
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("home")).unwrap();
        Some(Self { tmp })
    }

    pub fn state_dir(&self) -> PathBuf {
        self.tmp.path().join("state/carrova")
    }

    /// Remember the device of `seed` as `name`, as a code-paired move would.
    pub fn remember(&self, seed: u8, name: &str) -> DevicePublic {
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
    fn command(&self, args: &[&str]) -> Command {
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_carrova"));
        cmd.args(args)
            .env("HOME", self.tmp.path().join("home"))
            .env("XDG_STATE_HOME", self.tmp.path().join("state"))
            .env_remove("XDG_CONFIG_HOME")
            .env_remove("XDG_DATA_HOME")
            .env_remove("XDG_CACHE_HOME")
            .env_remove("RUST_LOG")
            .stdin(Stdio::null());
        cmd
    }

    /// `carrova ARGS` run to completion.
    pub fn carrova(&self, args: &[&str]) -> Output {
        self.command(args).output().unwrap()
    }

    /// `carrova ARGS` left running; killed when the result is dropped.
    pub fn spawn(&self, args: &[&str]) -> Background {
        let child = self
            .command(args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Background(child)
    }

    /// `carrova ARGS`, which must succeed; its stdout.
    pub fn ok(&self, args: &[&str]) -> String {
        let out = self.carrova(args);
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(out.status.success(), "carrova {args:?} failed: {stderr}");
        String::from_utf8(out.stdout).unwrap()
    }

    /// `carrova ARGS --json`, which must succeed; its stdout as JSON.
    pub fn json(&self, args: &[&str]) -> Value {
        let mut args = args.to_vec();
        args.push("--json");
        serde_json::from_str(&self.ok(&args)).expect("stdout is pure JSON")
    }
}

/// A `carrova` process left running; killed and reaped on drop, so a
/// failing test leaves no orphan.
pub struct Background(Child);

impl Background {
    /// Still running (it has not exited).
    pub fn running(&mut self) -> bool {
        self.0.try_wait().unwrap().is_none()
    }
}

impl Drop for Background {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
