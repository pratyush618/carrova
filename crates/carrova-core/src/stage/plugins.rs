//! Plugins over staged files. After staging verified an approved plan's
//! files, each staged file of a plugin item ([`Plan::plugins`]) is read,
//! passed through its plugin and the output staged next to it
//! (`<n>.plugin`, [`STAGED_MODE`], synced). [`Staged`] then points at the
//! output and records its digest, which apply journals instead of the
//! manifest's. The verified original stays, so a resumed LAN staging still
//! matches its checkpoints.
//!
//! The plan reserved room for each output at its original size
//! ([`crate::space`]); since a plugin may grow a file, each output is
//! checked against the free space (with the usual margin) before it is
//! written unless the caller lifted that guard, and a shortfall (or a
//! write hitting a full disk) is a [`CoreError::DiskFull`].

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use carrova_manifest::{Blake3Hash, Manifest};
use carrova_platform::{Platform, PlatformError, SemanticRoot};
use carrova_plugin::{MAX_IO_BYTES, Plugin, RunBudget, TransformContext};

use super::{STAGED_MODE, Staged};
use crate::error::{CoreError, Result};
use crate::key::RootedPath;
use crate::plan::Plan;
use crate::space::{disk_full_or, required_space};

/// Run every planned plugin over its items' staged files (see the module
/// docs); `check_room` keeps the free-space guard before each output.
/// A plugin failure is a [`CoreError::Plugin`], a full disk
/// [`CoreError::DiskFull`]; nothing outside staging has been written by
/// then. Rerunning on the `staged` it was given redoes the same outputs.
pub(crate) fn run_plugins(
    platform: &dyn Platform,
    plan: &Plan,
    manifest: &Manifest,
    plugins: &[Plugin],
    staged: &mut Staged,
    check_room: bool,
) -> Result<()> {
    if plan.plugins.is_empty() {
        return Ok(());
    }
    let context = TransformContext {
        source_home: manifest.source_home.clone().unwrap_or_default(),
        target_home: platform
            .resolve_root(SemanticRoot::Home)?
            .to_str()
            .ok_or_else(|| CoreError::UnsafePath {
                path: "home".into(),
                reason: "home folder path is not UTF-8".into(),
            })?
            .to_owned(),
        source_os: manifest.source.os,
        target_os: platform.os(),
    };
    for planned in &plan.plugins {
        let failed = |file: String, reason: String| CoreError::Plugin {
            plugin: planned.plugin.to_string(),
            file,
            reason,
        };
        let plugin = plugins
            .iter()
            .find(|p| p.manifest().id == planned.plugin)
            .ok_or_else(|| failed("-".into(), "not available on this computer".into()))?;
        let mut budget = RunBudget::new();
        for a in plan
            .actions
            .iter()
            .filter(|a| planned.items.contains(&a.item))
        {
            let key = (a.item.clone(), a.entry_index);
            let Some(src) = staged.files.get(&key) else {
                continue; // nothing to write for this action
            };
            let label = RootedPath::new(a.root, &a.rel).to_string();
            let output = plugin
                .transform(
                    platform,
                    &a.rel,
                    &read_capped(platform, src)?,
                    &context,
                    &mut budget,
                )
                .map_err(|e| failed(label, e.to_string()))?;
            let out = output_path(src);
            if check_room {
                ensure_room(platform, &out, output.len() as u64)?;
            }
            write_staged(platform, &out, &output)?;
            staged.files.insert(key.clone(), out);
            staged
                .rewritten
                .insert(key, (Blake3Hash::of(&output), output.len() as u64));
        }
    }
    Ok(())
}

/// A file's bytes, at most one byte over what a plugin accepts (so the
/// plugin reports the size, not this reader).
pub(crate) fn read_capped(platform: &dyn Platform, path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    platform
        .open_read_nofollow(path)?
        .take(MAX_IO_BYTES + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// [`CoreError::DiskFull`] unless the filesystem of `path` has room for
/// `bytes` plus the usual margin ([`required_space`]).
fn ensure_room(platform: &dyn Platform, path: &Path, bytes: u64) -> Result<()> {
    let dir = path.parent().unwrap_or(path);
    let needed = required_space(bytes);
    let available = platform.free_space(dir)?;
    if available < needed {
        return Err(CoreError::DiskFull { needed, available });
    }
    Ok(())
}

/// `<staged file>.plugin`.
fn output_path(src: &Path) -> PathBuf {
    let mut name = src.file_name().map(OsString::from).unwrap_or_default();
    name.push(".plugin");
    src.with_file_name(name)
}

/// Write `bytes` to a fresh staged file at `path` (a stale one from an
/// earlier run is replaced) and sync it.
fn write_staged(platform: &dyn Platform, path: &Path, bytes: &[u8]) -> Result<()> {
    match platform.remove_file(path) {
        Err(PlatformError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {}
        other => other?,
    }
    let mut file = platform.create_new_file(path, Some(STAGED_MODE))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|e| {
            let dir = path.parent().unwrap_or(path);
            disk_full_or(platform, dir, e, required_space(bytes.len() as u64))
        })
}
