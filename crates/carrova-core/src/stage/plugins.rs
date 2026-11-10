//! Plugins over staged files. After staging verified an approved plan's
//! files, each staged file of a plugin item ([`Plan::plugins`]) is read,
//! passed through its plugin and the output staged next to it
//! (`<n>.plugin`, [`STAGED_MODE`], synced). [`Staged`] then points at the
//! output and records its digest, which apply journals instead of the
//! manifest's. The verified original stays, so a resumed LAN staging still
//! matches its checkpoints.
//!
//! shortcut: the plan's free-space check counts the original sizes; output
//! (≤ `MAX_IO_BYTES` per file) may be larger — count it once a plugin that
//! grows files ships.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use carrova_manifest::{Blake3Hash, Manifest};
use carrova_platform::{Platform, PlatformError, SemanticRoot};
use carrova_plugin::{MAX_IO_BYTES, MAX_RUN_FUEL, Plugin, TransformContext};

use super::{STAGED_MODE, Staged};
use crate::error::{CoreError, Result};
use crate::key::RootedPath;
use crate::plan::Plan;

/// Run every planned plugin over its items' staged files (see the module
/// docs). Any failure is a [`CoreError::Plugin`]; nothing outside staging
/// has been written by then.
pub(crate) fn run_plugins(
    platform: &dyn Platform,
    plan: &Plan,
    manifest: &Manifest,
    plugins: &[Plugin],
    staged: &mut Staged,
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
        let mut budget = MAX_RUN_FUEL;
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
                .transform(&a.rel, &read_capped(platform, src)?, &context, &mut budget)
                .map_err(|e| failed(label, e.to_string()))?;
            let out = output_path(src);
            write_staged(platform, &out, &output)?;
            staged.files.insert(key.clone(), out);
            staged
                .rewritten
                .insert(key, (Blake3Hash::of(&output), output.len() as u64));
        }
    }
    Ok(())
}

/// A staged file's bytes, at most one byte over what a plugin accepts (so
/// the plugin reports the size, not this reader).
fn read_capped(platform: &dyn Platform, path: &Path) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    platform
        .open_read_nofollow(path)?
        .take(MAX_IO_BYTES + 1)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
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
    file.write_all(bytes)?;
    file.sync_all()?;
    Ok(())
}
