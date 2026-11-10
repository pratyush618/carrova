//! `carrova report`: opt-in local error reports, end to end through the
//! binary (HOME and Carrova's state dir in a temp dir).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use serde_json::Value;
use tempfile::TempDir;

/// One user: a temp HOME with its own state dir and an empty recipe tree.
struct User {
    tmp: TempDir,
}

impl User {
    fn new() -> Self {
        let tmp = TempDir::new().unwrap();
        std::fs::create_dir_all(tmp.path().join("home")).unwrap();
        std::fs::create_dir_all(tmp.path().join("recipes")).unwrap();
        Self { tmp }
    }

    fn home(&self) -> PathBuf {
        self.tmp.path().join("home")
    }

    fn reports_dir(&self) -> PathBuf {
        self.tmp.path().join("state/carrova/reports")
    }

    /// `carrova ARGS` as this user, stdin not a terminal.
    fn carrova(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_carrova"))
            .args(args)
            .env("HOME", self.home())
            .env("XDG_STATE_HOME", self.tmp.path().join("state"))
            .env("CARROVA_RECIPES", self.tmp.path().join("recipes"))
            .env("CARROVA_PASSPHRASE", "irrelevant")
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

    /// Fail a command with an error that names a file in HOME.
    fn fail_with_a_home_path(&self) -> Output {
        let missing = self.home().join("secret-plans.carrova");
        let out = self.carrova(&["import", missing.to_str().unwrap(), "--dry-run"]);
        assert!(!out.status.success());
        out
    }

    fn report_ids(&self) -> Vec<String> {
        self.json(&["report", "list"])
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["id"].as_str().unwrap().to_owned())
            .collect()
    }
}

#[test]
fn consent_starts_unset_and_nothing_is_recorded() {
    let user = User::new();

    assert_eq!(user.json(&["report", "consent"])["consent"], "unset");
    user.fail_with_a_home_path();

    assert!(user.report_ids().is_empty());
    assert!(!user.reports_dir().join("consent.json").exists());
    assert!(user.ok(&["report", "consent"]).contains("not chosen yet"));
}

#[test]
fn with_consent_an_error_exit_saves_a_redacted_report() {
    let user = User::new();
    user.ok(&["report", "consent", "allow"]);

    let out = user.fail_with_a_home_path();

    let stderr = String::from_utf8_lossy(&out.stderr);
    let ids = user.report_ids();
    assert_eq!(ids.len(), 1, "{stderr}");
    assert!(stderr.contains(&format!("carrova report show {}", ids[0])));
    let report = user.json(&["report", "show", &ids[0]]);
    assert_eq!(report["kind"], "error");
    assert_eq!(report["app_version"], env!("CARGO_PKG_VERSION"));
    let messages = report["message_chain"].to_string();
    assert!(messages.contains("~/<path>/*.carrova"), "{messages}");
    assert!(!messages.contains("secret-plans"), "{messages}");
    assert!(
        !messages.contains(user.home().to_str().unwrap()),
        "{messages}"
    );
}

#[test]
fn issue_prints_the_draft_and_a_prefilled_url_without_sending() {
    let user = User::new();
    user.ok(&["report", "consent", "allow"]);
    user.fail_with_a_home_path();
    let id = user.report_ids().remove(0);

    let text = user.ok(&["report", "issue", &id]);
    let draft = user.json(&["report", "issue", &id]);

    let url = draft["url"].as_str().unwrap();
    assert!(url.starts_with("https://github.com/"), "{url}");
    assert!(text.contains(url));
    assert!(text.contains(draft["title"].as_str().unwrap()));
    for line in draft["body"].as_str().unwrap().lines() {
        assert!(text.contains(line), "body line missing: {line}");
    }
}

#[test]
fn delete_and_clear_refuse_without_a_terminal_or_yes() {
    let user = User::new();
    user.ok(&["report", "consent", "allow"]);
    user.fail_with_a_home_path();
    user.fail_with_a_home_path();
    let ids = user.report_ids();

    for args in [
        vec!["report", "delete", ids[0].as_str()],
        vec!["report", "clear"],
    ] {
        let out = user.carrova(&args);
        assert!(!out.status.success());
        assert!(String::from_utf8_lossy(&out.stderr).contains("--yes"));
    }
    assert_eq!(user.report_ids().len(), 2, "nothing deleted");

    user.ok(&["report", "delete", &ids[0], "--yes"]);
    assert_eq!(user.report_ids(), [ids[1].clone()]);
    user.ok(&["report", "clear", "--yes"]);
    assert!(user.report_ids().is_empty());
    assert_eq!(user.json(&["report", "consent"])["consent"], "allowed");
}

#[test]
fn denying_deletes_stored_reports_only_after_confirmation() {
    let user = User::new();
    user.ok(&["report", "consent", "allow"]);
    user.fail_with_a_home_path();

    let refused = user.carrova(&["report", "consent", "deny"]);
    assert!(!refused.status.success());
    assert_eq!(user.report_ids().len(), 1);

    user.ok(&["report", "consent", "deny", "--yes"]);
    assert!(user.report_ids().is_empty());
    user.fail_with_a_home_path();
    assert!(user.report_ids().is_empty(), "denied: nothing recorded");
}

#[test]
fn an_invalid_report_id_is_refused() {
    let user = User::new();

    let out = user.carrova(&["report", "show", "../consent"]);

    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("invalid report id"));
}

#[test]
fn reports_and_the_consent_choice_are_private_to_the_user() {
    let user = User::new();
    user.ok(&["report", "consent", "allow"]);
    user.fail_with_a_home_path();
    let id = user.report_ids().remove(0);

    let platform = carrova_platform::current();
    for file in ["consent.json".to_owned(), format!("{id}.json")] {
        let meta = platform.metadata(&user.reports_dir().join(&file)).unwrap();
        assert_eq!(meta.mode, Some(0o600), "{file}");
    }
}
