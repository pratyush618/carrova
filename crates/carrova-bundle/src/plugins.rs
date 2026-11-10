//! Plugins: `plugins/<id>/` next to a recipe tree, [`PluginEntry`] blobs
//! in a bundle.

use std::fs;
use std::path::{Path, PathBuf};

use carrova_plugin::{Plugin, PluginManifest, PluginPackage};
use carrova_recipe::PluginId;
use serde::{Deserialize, Serialize};

use crate::{Asset, BlobKind, BlobRef, BundleError, Result};

/// A plugin's manifest inside `plugins/<id>/`.
pub const PLUGIN_MANIFEST_FILE: &str = "plugin.yaml";
/// A plugin's module inside `plugins/<id>/` (built by `just plugins`).
pub const PLUGIN_MODULE_FILE: &str = "plugin.wasm";

/// One plugin in the index: two blobs, its manifest and its module.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PluginEntry {
    /// Plugin id (unique in the index).
    pub id: PluginId,
    /// Plugin version, as its manifest says.
    pub version: String,
    /// The `plugin.yaml` blob.
    pub manifest: BlobRef,
    /// The WebAssembly module blob.
    pub module: BlobRef,
}

impl PluginEntry {
    /// Both blobs with their asset names.
    #[must_use]
    pub fn blobs(&self) -> [(Asset, BlobRef); 2] {
        [
            (
                Asset::Blob(BlobKind::PluginManifest, self.manifest.hash),
                self.manifest,
            ),
            (Asset::Blob(BlobKind::Plugin, self.module.hash), self.module),
        ]
    }
}

/// Where the plugins of the recipe tree at `recipes_dir` live: the
/// sibling `plugins/` directory (`recipes/` and `plugins/` in the repo).
#[must_use]
pub fn plugins_dir(recipes_dir: &Path) -> PathBuf {
    recipes_dir.join("..").join("plugins")
}

/// Every plugin next to the recipe tree at `recipes_dir`, sorted by id.
/// Each must parse, carry its directory's id and load (no imports, ABI
/// exports): a tree with a broken plugin is refused. No plugins directory
/// means no plugins.
pub(crate) fn load_plugin_tree(recipes_dir: &Path) -> Result<Vec<PluginPackage>> {
    let dir = plugins_dir(recipes_dir);
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(BundleError::io(&dir, e)),
    };
    let mut out = Vec::new();
    for entry in entries {
        let path = entry.map_err(|e| BundleError::io(&dir, e))?.path();
        if !path.is_dir() {
            continue;
        }
        let id = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let read = |name: &str| {
            let file = path.join(name);
            fs::read(&file).map_err(|e| BundleError::io(&file, e))
        };
        let package = package(&id, &read(PLUGIN_MANIFEST_FILE)?, read(PLUGIN_MODULE_FILE)?)?;
        Plugin::load(package.clone()).map_err(|e| invalid(&id, e.to_string()))?;
        out.push(package);
    }
    out.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    Ok(out)
}

/// The plugin of `entry` from its verified blobs. A manifest this build
/// cannot read, or one disagreeing with the entry, is an
/// [`BundleError::InvalidPlugin`].
pub(crate) fn bundled_plugin(
    entry: &PluginEntry,
    manifest: &[u8],
    wasm: Vec<u8>,
) -> Result<PluginPackage> {
    let package = package(entry.id.as_str(), manifest, wasm)?;
    if package.manifest.version != entry.version {
        return Err(invalid(
            entry.id.as_str(),
            format!(
                "manifest version {} differs from the index",
                package.manifest.version
            ),
        ));
    }
    Ok(package)
}

/// Parse `manifest` and pair it with `wasm`; its id must be `id`.
fn package(id: &str, manifest: &[u8], wasm: Vec<u8>) -> Result<PluginPackage> {
    let text = std::str::from_utf8(manifest)
        .map_err(|_| invalid(id, "plugin.yaml is not UTF-8".into()))?;
    let manifest = PluginManifest::parse(text).map_err(|e| invalid(id, e.to_string()))?;
    if manifest.id.as_str() != id {
        return Err(invalid(id, format!("manifest names id `{}`", manifest.id)));
    }
    Ok(PluginPackage { manifest, wasm })
}

fn invalid(id: &str, reason: String) -> BundleError {
    BundleError::InvalidPlugin {
        id: id.to_owned(),
        reason,
    }
}
