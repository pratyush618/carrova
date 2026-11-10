//! End-to-end tests of `carrova recipes status | rollback` and of the
//! unverified-recipes warning. HOME points into a temp dir so the user's
//! real recipe store is never touched.

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

use anyhow::Result;
use tempfile::TempDir;

const RECIPE: &str = "schema: 1\nid: demo\nname: Demo\ncategory: dev-tools\n\
    detect:\n  linux:\n    - binary: demo\n\
    items:\n  - id: config\n    label: Config\n    paths: [{ root: user_config, rel: demo }]\n";

/// A temp dir holding `home/` and a one-recipe tree `recipes/`.
fn fixture() -> Result<TempDir> {
    let tmp = TempDir::new()?;
    let dir = tmp.path().join("recipes/dev-tools/demo");
    fs::create_dir_all(&dir)?;
    fs::write(dir.join("recipe.yaml"), RECIPE)?;
    fs::create_dir_all(tmp.path().join("home"))?;
    Ok(tmp)
}

fn carrova(tmp: &Path, args: &[&str]) -> Result<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_carrova"))
        .args(args)
        .env("HOME", tmp.join("home"))
        .env_remove("XDG_DATA_HOME")
        .env_remove("CARROVA_RECIPES")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .output()?)
}

fn stderr(out: &Output) -> String {
    String::from_utf8_lossy(&out.stderr).into_owned()
}

#[test]
fn status_with_dev_recipes_warns_once_that_they_are_unverified() -> Result<()> {
    let tmp = fixture()?;
    let dir = tmp.path().join("recipes");
    let out = carrova(
        tmp.path(),
        &["--recipes", &dir.to_string_lossy(), "recipes", "status"],
    )?;
    let (stdout, stderr) = (String::from_utf8(out.stdout.clone())?, stderr(&out));
    assert!(out.status.success(), "{stderr}");
    assert_eq!(stderr.matches("unverified recipes").count(), 1, "{stderr}");
    assert!(stdout.contains("developer recipe tree"), "{stdout}");
    assert!(stdout.contains("Recipes  1 (1 unverified)"), "{stdout}");
    Ok(())
}

#[test]
fn carrova_recipes_in_the_environment_is_a_warned_dev_override() -> Result<()> {
    let tmp = fixture()?;
    let out = Command::new(env!("CARGO_BIN_EXE_carrova"))
        .args(["recipes", "list"])
        .env("HOME", tmp.path().join("home"))
        .env("CARROVA_RECIPES", tmp.path().join("recipes"))
        .env_remove("XDG_DATA_HOME")
        .env_remove("RUST_LOG")
        .stdin(Stdio::null())
        .output()?;
    let (stdout, stderr) = (String::from_utf8(out.stdout.clone())?, stderr(&out));
    assert!(out.status.success(), "{stderr}");
    assert!(stderr.contains("unverified recipes"), "{stderr}");
    assert!(stdout.contains("demo"), "{stdout}");
    assert!(stdout.contains("unverified"), "{stdout}");
    Ok(())
}

#[test]
fn status_json_is_pure_json_on_stdout() -> Result<()> {
    let tmp = fixture()?;
    let dir = tmp.path().join("recipes");
    let out = carrova(
        tmp.path(),
        &[
            "--recipes",
            &dir.to_string_lossy(),
            "recipes",
            "status",
            "--json",
        ],
    )?;
    assert!(out.status.success(), "{}", stderr(&out));
    let v: serde_json::Value = serde_json::from_slice(&out.stdout)?;
    assert_eq!(v["source"]["kind"], "dev_dir");
    assert_eq!(v["verified"], false);
    assert_eq!(v["recipes"]["total"], 1);
    Ok(())
}

#[test]
fn rollback_without_a_terminal_needs_yes() -> Result<()> {
    let tmp = fixture()?;
    let out = carrova(tmp.path(), &["recipes", "rollback"])?;
    let stderr = stderr(&out);
    assert!(!out.status.success());
    assert!(stderr.contains("re-run with --yes"), "{stderr}");
    Ok(())
}

#[test]
#[cfg(unix)] // Windows finds the store via known folders, not HOME: --yes would roll back the real one
fn rollback_with_nothing_installed_explains_why() -> Result<()> {
    let tmp = fixture()?;
    let out = carrova(tmp.path(), &["recipes", "rollback", "--yes"])?;
    let stderr = stderr(&out);
    assert!(!out.status.success());
    assert!(
        stderr.contains("no recipe bundle is installed; nothing to roll back"),
        "{stderr}"
    );
    Ok(())
}
