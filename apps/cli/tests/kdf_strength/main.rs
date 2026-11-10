//! `carrova export --kdf-strength`: the desktop's archive-protection
//! choice, with no weaker level than the default.
#![allow(clippy::unwrap_used)]

use std::process::{Command, Output};

fn carrova(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_carrova"))
        .args(args)
        .env_remove("CARROVA_PASSPHRASE")
        .output()
        .unwrap()
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn export_offers_exactly_the_engine_strengths() {
    let help = String::from_utf8_lossy(&carrova(&["export", "--help"]).stdout).into_owned();
    assert!(help.contains("--kdf-strength <LEVEL>"), "{help}");
    for level in ["standard", "strong", "maximum"] {
        assert!(help.contains(level), "{level}: {help}");
    }
}

#[test]
fn a_level_below_the_default_does_not_exist() {
    let out = carrova(&[
        "export",
        "-o",
        "x.carrova",
        "--kdf-strength",
        "weak",
        "--yes",
    ]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("invalid value 'weak'"),
        "{}",
        stderr(&out)
    );
}

#[test]
fn strength_and_raw_memory_are_one_choice() {
    let out = carrova(&[
        "export",
        "-o",
        "x.carrova",
        "--kdf-strength",
        "strong",
        "--kdf-memory",
        "64",
        "--yes",
    ]);
    assert!(!out.status.success());
    assert!(
        stderr(&out).contains("cannot be used with"),
        "{}",
        stderr(&out)
    );
}
