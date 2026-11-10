//! End-to-end `carrova recipes reset`: confirmation rules. HOME points into
//! a temp dir so the user's real recipe store is never touched.

use std::process::{Command, Output, Stdio};

use anyhow::Result;
use tempfile::TempDir;

fn carrova(home: &TempDir, args: &[&str]) -> Result<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_carrova"))
        .args(args)
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env_remove("CARROVA_RECIPES")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .output()?)
}

#[test]
fn reset_without_a_terminal_needs_yes() -> Result<()> {
    let home = TempDir::new()?;
    let out = carrova(&home, &["recipes", "reset"])?;
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(!out.status.success());
    assert!(stderr.contains("re-run with --yes"), "{stderr}");
    Ok(())
}

#[test]
#[cfg(unix)] // Windows finds the store via known folders, not HOME: --yes would reset the real one
fn reset_with_nothing_installed_says_so() -> Result<()> {
    let home = TempDir::new()?;
    let out = carrova(&home, &["recipes", "reset", "--yes"])?;
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        String::from_utf8(out.stdout)?,
        "No recipe bundle was installed; nothing to reset.\n"
    );
    Ok(())
}
