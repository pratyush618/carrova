//! End-to-end over loopback: `carrova send` on one fake machine, `carrova
//! receive` on another (two temp HOMEs with their own XDG and state dirs),
//! both as real `carrova` processes. The receiver uses the sender's
//! `--json` pairing record with a `127.0.0.1` ticket, so discovery is not
//! needed. Linux only (returns early elsewhere).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use carrova_platform::OsKind;
use tempfile::TempDir;

const SSH_CONFIG: &str = "Host box\n  User me\n";

/// One fake machine.
struct Machine {
    tmp: TempDir,
}

impl Machine {
    fn new() -> Self {
        let m = Self {
            tmp: TempDir::new().unwrap(),
        };
        for d in [
            "home/.config",
            "home/.local/share",
            "home/.cache",
            "state",
            "bin",
        ] {
            fs::create_dir_all(m.tmp.path().join(d)).unwrap();
        }
        m
    }

    fn home(&self) -> PathBuf {
        self.tmp.path().join("home")
    }

    fn state(&self) -> PathBuf {
        self.tmp.path().join("state")
    }

    fn carrova(&self, args: &[&str]) -> Command {
        let home = self.home();
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_carrova"));
        cmd.arg("--recipes")
            .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/../../recipes"))
            .args(args)
            .env_remove("RUST_LOG")
            .env_remove("CARROVA_RECIPES")
            .env_remove("CARROVA_PASSPHRASE")
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("XDG_STATE_HOME", self.state())
            .env("PATH", self.tmp.path().join("bin"))
            .current_dir(self.tmp.path())
            .stdin(Stdio::null());
        cmd
    }
}

/// Kills the child if the test fails before it exits.
struct Reaped(Child);

impl Drop for Reaped {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Every `log.jsonl` below `dir`, concatenated.
fn logs(dir: &Path) -> String {
    let mut all = String::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            all.push_str(&logs(&path));
        } else if path.file_name().is_some_and(|n| n == "log.jsonl") {
            all.push_str(&fs::read_to_string(&path).unwrap());
        }
    }
    all
}

#[test]
fn send_receive_moves_file_and_never_logs_the_code() {
    if carrova_platform::current().os() != OsKind::Linux {
        return;
    }
    let src = Machine::new();
    fs::create_dir_all(src.home().join(".ssh")).unwrap();
    fs::write(src.home().join(".ssh/config"), SSH_CONFIG).unwrap();
    let dst = Machine::new();

    let mut sender = Reaped(
        src.carrova(&["send", "--include", "ssh/config", "--yes", "--json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap(),
    );
    // Drain stderr concurrently so a full pipe never blocks the sender.
    let mut stderr = sender.0.stderr.take().unwrap();
    let sender_err = std::thread::spawn(move || {
        let mut text = String::new();
        stderr.read_to_string(&mut text).unwrap();
        text
    });
    let mut stdout = BufReader::new(sender.0.stdout.take().unwrap());
    let mut first = String::new();
    stdout.read_line(&mut first).unwrap();
    let record: serde_json::Value = serde_json::from_str(&first)
        .unwrap_or_else(|e| panic!("first stdout line is the pairing record ({e}): {first:?}"));
    let code = record["pairing_code"].as_str().unwrap().to_owned();
    let ticket = record["ticket"].as_str().expect("fallback ticket shown");
    let port = ticket.rsplit_once(':').unwrap().1;
    let local = format!("{code}@127.0.0.1:{port}");

    let received = dst
        .carrova(&[
            "receive",
            &local,
            "--confirm-sensitive",
            "ssh/config",
            "--yes",
            "--json",
        ])
        .output()
        .unwrap();
    let received_out = String::from_utf8_lossy(&received.stdout);
    let received_err = String::from_utf8_lossy(&received.stderr);
    assert!(
        received.status.success(),
        "receive failed\nstdout:\n{received_out}\nstderr:\n{received_err}"
    );
    let result: serde_json::Value = serde_json::from_str(&received_out).unwrap();
    assert_eq!(result["status"], "applied", "{result}");

    let status = sender.0.wait().unwrap();
    let mut rest = String::new();
    stdout.read_to_string(&mut rest).unwrap();
    let sender_err = sender_err.join().unwrap();
    assert!(
        status.success(),
        "send failed\nstdout:\n{rest}\nstderr:\n{sender_err}"
    );
    let sent: serde_json::Value = serde_json::from_str(&rest).unwrap();
    assert_eq!(sent["status"], "sent", "{sent}");
    assert!(
        sent["checklist"]["moved"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m == "ssh/config"),
        "{sent}"
    );

    assert_eq!(
        fs::read_to_string(dst.home().join(".ssh/config")).unwrap(),
        SSH_CONFIG
    );
    // Events (stderr) and both session logs never carry the code.
    for (what, text) in [
        ("sender log", logs(&src.state())),
        ("receiver log", logs(&dst.state())),
        ("sender events", sender_err),
        ("receiver events", received_err.into_owned()),
    ] {
        assert!(!text.is_empty(), "{what} is empty");
        assert!(!text.contains(&code), "{what} contains the pairing code");
    }
}
