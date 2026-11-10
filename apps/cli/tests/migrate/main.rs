//! End-to-end: `carrova export` on one fake machine, `carrova import` on
//! another (two temp HOMEs with their own XDG dirs and state dir), using the
//! repo's recipes. Linux only (returns early elsewhere).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use carrova_platform::OsKind;
use tempfile::TempDir;

const PASS: &str = "correct horse battery staple";
const KEY: &str = "-----BEGIN OPENSSH PRIVATE KEY-----\nfake\n-----END OPENSSH PRIVATE KEY-----\n";

// Modes via the platform adapter (no OS-specific code outside carrova-platform).
fn set_mode(p: &Path, mode: u32) {
    carrova_platform::current().set_mode(p, mode).unwrap();
}

fn mode_of(p: &Path) -> u32 {
    carrova_platform::current()
        .metadata(p)
        .unwrap()
        .mode
        .unwrap()
        & 0o7777
}

fn recipes() -> String {
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../recipes").to_owned()
}

/// One fake machine.
struct Machine {
    tmp: TempDir,
}

impl Machine {
    fn new() -> Option<Self> {
        if carrova_platform::current().os() != OsKind::Linux {
            return None;
        }
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
        Some(m)
    }

    fn home(&self) -> PathBuf {
        self.tmp.path().join("home")
    }

    fn write(&self, rel: &str, text: &str, mode: u32) {
        let p = self.home().join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, text).unwrap();
        set_mode(&p, mode);
    }

    fn read(&self, rel: &str) -> Option<String> {
        fs::read_to_string(self.home().join(rel)).ok()
    }

    fn mode(&self, rel: &str) -> u32 {
        mode_of(&self.home().join(rel))
    }

    fn carrova(&self, args: &[&str]) -> Output {
        self.carrova_with_pass(args, PASS)
    }

    fn carrova_with_pass(&self, args: &[&str], pass: &str) -> Output {
        let home = self.home();
        Command::new(env!("CARGO_BIN_EXE_carrova"))
            .arg("--recipes")
            .arg(recipes())
            .args(args)
            .env_remove("RUST_LOG")
            .env_remove("CARROVA_RECIPES")
            .env("HOME", &home)
            .env("XDG_CONFIG_HOME", home.join(".config"))
            .env("XDG_DATA_HOME", home.join(".local/share"))
            .env("XDG_CACHE_HOME", home.join(".cache"))
            .env("XDG_STATE_HOME", self.tmp.path().join("state"))
            .env("PATH", self.tmp.path().join("bin"))
            .env("CARROVA_PASSPHRASE", pass)
            .current_dir(self.tmp.path())
            .output()
            .unwrap()
    }

    /// Run and require success; returns stdout.
    fn ok(&self, args: &[&str]) -> String {
        let out = self.carrova(args);
        let stdout = String::from_utf8_lossy(&out.stdout).into_owned();
        assert!(
            out.status.success(),
            "carrova {args:?} failed\nstdout:\n{stdout}\nstderr:\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        stdout
    }
}

/// Sender with ssh config + a private key; returns (sender, archive path).
fn exported() -> Option<(Machine, PathBuf)> {
    let src = Machine::new()?;
    fs::create_dir_all(src.home().join(".ssh")).unwrap();
    set_mode(&src.home().join(".ssh"), 0o700);
    src.write(".ssh/config", "Host box\n  User me\n", 0o644);
    src.write(".ssh/id_test", KEY, 0o600);
    let archive = src.tmp.path().join("out.carrova");
    let a = archive.to_str().unwrap();
    let stdout = src.ok(&[
        "export",
        "-o",
        a,
        "--include",
        "ssh/keys",
        "--confirm-sensitive",
        "ssh/keys",
        "--kdf-memory",
        "64",
        "--yes",
        "--json",
    ]);
    let v: serde_json::Value = serde_json::from_str(&stdout).expect("stdout is pure JSON");
    assert!(v["bytes"].as_u64().unwrap() > 0, "{v}");
    let moved: Vec<&str> = v["checklist"]["moved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap())
        .collect();
    assert!(moved.contains(&"ssh/keys"), "{v}");
    assert!(moved.contains(&"ssh/config"), "{v}");
    assert_eq!(mode_of(&archive), 0o600, "archive mode");
    Some((src, archive))
}

fn path(p: &Path) -> &str {
    p.to_str().unwrap()
}

#[test]
fn export_dry_run_import_rollback() {
    let Some((_src, archive)) = exported() else {
        return;
    };
    let dst = Machine::new().unwrap();

    // Dry run: plan printed, nothing written into HOME.
    let plan = dst.ok(&[
        "import",
        path(&archive),
        "--dry-run",
        "--confirm-sensitive",
        "ssh/keys",
        "--confirm-sensitive",
        "ssh/config",
    ]);
    assert!(plan.contains("Import plan"), "{plan}");
    assert!(plan.contains("home:.ssh/id_test"), "{plan}");
    assert!(plan.contains("Dry run: nothing was written."), "{plan}");
    assert!(!dst.home().join(".ssh").exists(), "dry run wrote into HOME");

    // Apply.
    let out = dst.ok(&[
        "import",
        path(&archive),
        "--confirm-sensitive",
        "ssh/keys",
        "--confirm-sensitive",
        "ssh/config",
        "--yes",
    ]);
    assert!(out.contains("Moved"), "{out}");
    assert_eq!(
        dst.read(".ssh/config").as_deref(),
        Some("Host box\n  User me\n")
    );
    assert_eq!(dst.read(".ssh/id_test").as_deref(), Some(KEY));
    assert_eq!(dst.mode(".ssh/id_test"), 0o600);
    assert_eq!(dst.mode(".ssh/config"), 0o644);

    // Sessions list shows the import with a rollback point.
    let list = dst.ok(&["sessions", "list", "--json"]);
    let v: serde_json::Value = serde_json::from_str(&list).unwrap();
    let import = v
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["role"] == "import")
        .expect("import session listed");
    assert_eq!(import["state"], "done");
    let id = import["id"].as_str().unwrap().to_owned();
    let shown = dst.ok(&["sessions", "show", &id]);
    assert!(shown.contains(&id), "{shown}");

    // Rollback restores the receiver.
    let rb = dst.ok(&["rollback", &id, "--yes"]);
    assert!(rb.contains("Rolled back"), "{rb}");
    assert!(dst.read(".ssh/id_test").is_none());
    assert!(dst.read(".ssh/config").is_none());
}

#[test]
fn sensitive_not_imported_without_confirmation() {
    let Some((_src, archive)) = exported() else {
        return;
    };
    let dst = Machine::new().unwrap();
    let plan = dst.ok(&["import", path(&archive), "--dry-run"]);
    assert!(
        plan.lines().any(|l| l.starts_with("  [ ] ssh/keys")
            && l.contains("[sensitive (")
            && l.ends_with("needs --confirm-sensitive ssh/keys]")),
        "{plan}"
    );
    // The receiver itself treats ssh/config as sensitive (it controls SSH
    // logins): confirm only that one.
    let out = dst.ok(&[
        "import",
        path(&archive),
        "--confirm-sensitive",
        "ssh/config",
        "--yes",
        "--json",
    ]);
    let v: serde_json::Value = serde_json::from_str(&out).expect("stdout is pure JSON");
    assert_eq!(v["status"], "applied", "{v}");
    assert!(dst.read(".ssh/config").is_some());
    assert!(
        dst.read(".ssh/id_test").is_none(),
        "sensitive item imported unconfirmed"
    );
}

#[test]
fn keep_both_keeps_existing() {
    let Some((_src, archive)) = exported() else {
        return;
    };
    let dst = Machine::new().unwrap();
    dst.write(".ssh/config", "Host mine\n", 0o600);
    let out = dst.ok(&[
        "import",
        path(&archive),
        "--rule",
        "default=keep-both",
        "--keep-both-suffix",
        "incoming",
        "--confirm-sensitive",
        "ssh/config",
        "--yes",
    ]);
    assert!(out.contains("Moved"), "{out}");
    assert_eq!(dst.read(".ssh/config").as_deref(), Some("Host mine\n"));
    assert_eq!(
        dst.read(".ssh/config.incoming").as_deref(),
        Some("Host box\n  User me\n")
    );
}

#[test]
fn empty_passphrase_refused_on_export() {
    let Some(src) = Machine::new() else {
        return;
    };
    src.write(".ssh/config", "Host box\n", 0o644);
    let archive = src.tmp.path().join("out.carrova");
    let out = src.carrova_with_pass(&["export", "-o", path(&archive), "--yes"], "");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("non-empty passphrase"), "{stderr}");
    assert!(!archive.exists(), "archive written with empty passphrase");
}

#[test]
fn weak_passphrase_refused_on_export() {
    let Some(src) = Machine::new() else {
        return;
    };
    src.write(".ssh/config", "Host box\n", 0o644);
    let archive = src.tmp.path().join("out.carrova");
    let out = src.carrova_with_pass(&["export", "-o", path(&archive), "--yes"], "hunter2");
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("the passphrase is weak; an archive needs at least a fair one"),
        "{stderr}"
    );
    assert!(!stderr.contains("hunter2"), "{stderr}");
    assert!(!archive.exists(), "archive written with a weak passphrase");
}

#[test]
fn io_error_cause_printed_once() {
    let Some(dst) = Machine::new() else {
        return;
    };
    let missing = dst.tmp.path().join("missing.carrova");
    let out = dst.carrova(&["import", path(&missing), "--dry-run"]);
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert_eq!(
        stderr.matches("No such file or directory").count(),
        1,
        "{stderr}"
    );
}

#[test]
fn wrong_passphrase_fails() {
    let Some((_src, archive)) = exported() else {
        return;
    };
    let dst = Machine::new().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_carrova"))
        .args([
            "--recipes",
            &recipes(),
            "import",
            path(&archive),
            "--dry-run",
        ])
        .env("HOME", dst.home())
        .env("XDG_STATE_HOME", dst.tmp.path().join("state"))
        .env("CARROVA_PASSPHRASE", "wrong")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        !stderr.contains(PASS) && !stderr.contains("wrong\n"),
        "{stderr}"
    );
}

#[cfg(feature = "failpoints")]
#[test]
fn failure_mid_apply_rolls_back() {
    let Some((_src, archive)) = exported() else {
        return;
    };
    let dst = Machine::new().unwrap();
    dst.write(".ssh/config", "Host mine\n", 0o600);
    let out = dst.carrova(&[
        "import",
        path(&archive),
        "--confirm-sensitive",
        "ssh/keys",
        "--fail-after",
        "1",
        "--yes",
        "--json",
    ]);
    assert!(!out.status.success());
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).expect("stdout is pure JSON");
    assert_eq!(v["status"], "rolled_back", "{v}");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("rolled back"), "{stderr}");
    assert_eq!(dst.read(".ssh/config").as_deref(), Some("Host mine\n"));
    assert_eq!(dst.mode(".ssh/config"), 0o600);
    assert!(dst.read(".ssh/id_test").is_none());
}
