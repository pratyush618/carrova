//! Argument parsing and pre-engine validation of the session commands.
//! None of these reach the engine: they fail (or print help) first.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::process::{Command, Output};

fn carrova(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_carrova"))
        .args(args)
        .env_remove("RUST_LOG")
        .env_remove("CARROVA_PASSPHRASE")
        // stdin is not a terminal under `output()` (it's closed).
        .output()
        .unwrap()
}

fn fails_with(args: &[&str], needle: &str) {
    let out = carrova(args);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "{args:?} should fail");
    assert!(stderr.contains(needle), "{args:?}: {stderr}");
    assert!(out.stdout.is_empty(), "{args:?} printed to stdout");
}

#[test]
fn bad_rule_syntax() {
    fails_with(
        &["import", "a.carrova", "--rule", "git/config"],
        "expected default=RULE",
    );
    fails_with(
        &["import", "a.carrova", "--rule", "git/config=overwrite"],
        "unknown rule `overwrite`",
    );
    fails_with(
        &["import", "a.carrova", "--rule", "git/config:nowhere:x=skip"],
        "neither an item key nor ROOT:REL",
    );
}

#[test]
fn bad_remap() {
    fails_with(
        &["import", "a.carrova", "--remap", "git/config=nowhere"],
        "unknown root `nowhere`",
    );
    fails_with(
        &["import", "a.carrova", "--remap", "git/config"],
        "expected ITEM=ROOT",
    );
    fails_with(
        &["import", "a.carrova", "--remap", "git/config=home:"],
        "prefix after `:` is empty",
    );
}

#[test]
fn bad_exclude_path() {
    fails_with(
        &["export", "-o", "x", "--exclude-path", "ssh/keys=.ssh/id"],
        "is not ROOT:REL",
    );
}

#[test]
fn dry_run_and_yes_conflict() {
    fails_with(
        &["import", "a.carrova", "--dry-run", "--yes"],
        "cannot be used with",
    );
}

#[test]
fn missing_yes_without_tty() {
    for args in [
        &["export", "-o", "x.carrova"][..],
        &["import", "a.carrova"],
        &["rollback"],
        &["sessions", "dismiss", "20261108-101500-a1b2c3"],
        &["send"],
        &["receive", "7-acid-lemon"],
    ] {
        fails_with(args, "re-run with --yes");
    }
}

#[test]
fn bad_session_id() {
    fails_with(&["sessions", "show", "../etc"], "is not a session id");
    fails_with(&["rollback", "a/b"], "is not a session id");
}

#[test]
fn fail_after_is_hidden() {
    let out = carrova(&["import", "--help"]);
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(out.status.success());
    assert!(help.contains("--dry-run"), "{help}");
    assert!(!help.contains("fail-after"), "{help}");
}

#[test]
fn passphrase_env_not_shown_in_help() {
    let out = Command::new(env!("CARGO_BIN_EXE_carrova"))
        .args(["export", "--help"])
        .env("CARROVA_PASSPHRASE", "hunter2-secret")
        .output()
        .unwrap();
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("CARROVA_PASSPHRASE"), "{help}");
    assert!(!help.contains("hunter2-secret"), "{help}");
}

#[test]
fn import_trust_flag_documented() {
    let out = carrova(&["import", "--help"]);
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("--import-trust"), "{help}");
    assert!(help.contains("ownertrust"), "{help}");
}

#[test]
fn import_help_documents_keep_group_write() {
    let out = carrova(&["import", "--help"]);
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("--keep-group-write"), "{help}");
    assert!(help.contains("umask-002"), "{help}");
    assert!(
        help.contains("World-write, setuid and setgid are never restored"),
        "{help}"
    );
}

#[test]
fn receive_bad_code_fails_without_echoing_it() {
    let out = carrova(&["receive", "7-acid-lemonade", "--yes"]);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(stderr.contains("not a pairing code"), "{stderr}");
    assert!(!stderr.contains("lemon"), "{stderr}");
    assert!(out.stdout.is_empty());
}

#[test]
fn receive_without_code_or_tty_asks_for_the_argument() {
    fails_with(&["receive", "--yes"], "pass it: `carrova receive <code>`");
}

#[test]
fn send_resume_conflicts_with_new_selection() {
    fails_with(
        &["send", "--resume", "--include", "git/config"],
        "cannot be used with",
    );
}

#[test]
fn send_help_documents_json_pairing_code_record() {
    let out = carrova(&["send", "--help"]);
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    assert!(help.contains("pairing_code"), "{help}");
    assert!(help.contains("--confirm-sensitive"), "{help}");
}

#[test]
fn export_and_send_take_allow_plugin() {
    for cmd in ["export", "send"] {
        let out = carrova(&[cmd, "--help"]);
        assert!(out.status.success());
        let help = String::from_utf8_lossy(&out.stdout);
        assert!(help.contains("--allow-plugin <ID>"), "{cmd}: {help}");
    }
}

#[test]
fn receive_help_shares_import_plan_flags() {
    let out = carrova(&["receive", "--help"]);
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    for flag in [
        "--dry-run",
        "--rule",
        "--remap",
        "--keep-group-write",
        "--import-trust",
        "--resume",
    ] {
        assert!(help.contains(flag), "{flag}: {help}");
    }
    assert!(!help.contains("fail-after"), "{help}");
}

#[test]
fn paths_conflict_with_explicit_preset() {
    fails_with(
        &["export", "-o", "x", "--preset", "minimal", "/tmp/notes"],
        "cannot be used with",
    );
}

#[test]
fn paths_conflict_with_include() {
    fails_with(
        &["send", "--include", "git/config", "/tmp/notes"],
        "cannot be used with",
    );
}

#[test]
fn paths_conflict_with_exclude() {
    fails_with(
        &["export", "-o", "x", "--exclude", "git/config", "/tmp/notes"],
        "cannot be used with",
    );
}

#[test]
fn paths_allowed_with_default_preset_and_exclude_path_confirm_sensitive() {
    // Parsing succeeds: it fails later, at the missing --yes.
    fails_with(
        &[
            "export",
            "-o",
            "x",
            "/tmp/notes",
            "--exclude-path",
            "raw:home/notes=home:notes/a",
            "--confirm-sensitive",
            "/tmp/notes",
            "--force-unmovable",
            "raw:home/notes",
        ],
        "re-run with --yes",
    );
}

#[test]
fn send_resume_conflicts_with_paths() {
    fails_with(&["send", "/tmp/notes", "--resume"], "cannot be used with");
}

#[test]
fn integrate_help_lists_on_off_status() {
    let out = carrova(&["integrate", "file-manager", "--help"]);
    assert!(out.status.success());
    let help = String::from_utf8_lossy(&out.stdout);
    for state in ["on", "off", "status"] {
        assert!(help.contains(state), "{state}: {help}");
    }
}
