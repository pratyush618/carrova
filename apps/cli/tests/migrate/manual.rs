//! Manual mode: `carrova export PATH...` picks exact files and folders;
//! the receiver brings them with `--accept-safe-picks`.

use std::path::PathBuf;

use super::{KEY, Machine, path};

/// What the export checklist reports as moved (raw items as `root/rel`).
fn moved(stdout: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(stdout).expect("stdout is pure JSON");
    v["checklist"]["moved"]
        .as_array()
        .unwrap()
        .iter()
        .map(|m| m.as_str().unwrap().to_owned())
        .collect()
}

/// A sender with a project folder, a notes file, an SSH key and recipe data
/// Auto would suggest (`.ssh/config`).
fn sender() -> Option<(Machine, PathBuf)> {
    let src = Machine::new()?;
    src.write("proj/a.txt", "a\n", 0o644);
    src.write("proj/sub/b.txt", "b\n", 0o644);
    src.write("notes.txt", "notes\n", 0o644);
    src.write(".ssh/config", "Host box\n", 0o644);
    src.write(".ssh/id_test", KEY, 0o600);
    let archive = src.tmp.path().join("out.carrova");
    Some((src, archive))
}

fn export_args<'a>(archive: &'a str, rest: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "export",
        "-o",
        archive,
        "--kdf-memory",
        "64",
        "--yes",
        "--json",
    ];
    args.extend_from_slice(rest);
    args
}

#[test]
fn export_paths_writes_archive_with_picks_only() {
    let Some((src, archive)) = sender() else {
        return;
    };
    let proj = src.home().join("proj");
    // Relative paths resolve against the working directory (the machine's tmp dir).
    let stdout = src.ok(&export_args(
        path(&archive),
        &[path(&proj), "home/notes.txt"],
    ));
    assert_eq!(moved(&stdout), ["home/notes.txt", "home/proj"]);
}

#[test]
fn export_path_outside_roots_fails_with_plain_words() {
    let Some((src, archive)) = sender() else {
        return;
    };
    let outside = src.tmp.path().join("bin");
    let out = src.carrova(&export_args(path(&archive), &[path(&outside)]));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(
        stderr.contains(&format!(
            "cannot pick {}: it is outside your home and app folders",
            outside.display()
        )),
        "{stderr}"
    );
    assert!(!archive.exists(), "archive written despite a rejected pick");
}

#[test]
fn export_sensitive_pick_without_confirmation_fails_noninteractive() {
    let Some((src, archive)) = sender() else {
        return;
    };
    let key = src.home().join(".ssh/id_test");
    let out = src.carrova(&export_args(path(&archive), &[path(&key)]));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success(), "--yes confirmed a sensitive pick");
    assert!(stderr.contains("--confirm-sensitive"), "{stderr}");
    assert!(!archive.exists());
}

#[test]
fn export_sensitive_pick_confirmed_by_path_succeeds() {
    let Some((src, archive)) = sender() else {
        return;
    };
    let key = src.home().join(".ssh/id_test");
    let stdout = src.ok(&export_args(
        path(&archive),
        &[path(&key), "--confirm-sensitive", path(&key)],
    ));
    assert_eq!(moved(&stdout), ["home/.ssh/id_test"]);
}

#[test]
fn import_accept_safe_picks_brings_raw_items() {
    let Some((src, archive)) = sender() else {
        return;
    };
    let proj = src.home().join("proj");
    let notes = src.home().join("notes.txt");
    src.ok(&export_args(path(&archive), &[path(&proj), path(&notes)]));

    // Picks arrive unticked.
    let plain = Machine::new().unwrap();
    let _ = plain.carrova(&["import", path(&archive), "--yes"]);
    assert!(plain.read("notes.txt").is_none(), "pick restored unticked");

    let dst = Machine::new().unwrap();
    dst.ok(&["import", path(&archive), "--accept-safe-picks", "--yes"]);
    assert_eq!(dst.read("notes.txt").as_deref(), Some("notes\n"));
    assert_eq!(dst.read("proj/sub/b.txt").as_deref(), Some("b\n"));
}
