//! Removing indexes and blobs no live version uses.

use std::collections::HashSet;
use std::ffi::OsString;

use super::BundleStore;
use super::layout::StoreState;
use crate::{Asset, BundleIndex, Result};

impl BundleStore<'_> {
    /// Remove indexes and blobs no live version uses. Callers hold the
    /// store lock. Failures are logged: leftovers waste space but never
    /// affect correctness.
    pub(super) fn prune(&self) {
        if let Err(e) = self.try_prune() {
            tracing::warn!(error = %e, "recipe store cleanup failed");
        }
    }

    fn try_prune(&self) -> Result<()> {
        let state = StoreState::read(self.fs, &self.layout)?;
        let mut keep_dirs = HashSet::new();
        let mut keep_blobs = HashSet::new();
        for version in state.live() {
            keep_dirs.insert(OsString::from(version.0.to_string()));
            // Only deciding what to keep: no signature check needed.
            let index = BundleIndex::parse(&self.read_index_file(version, &Asset::Index)?)?;
            let all = index.recipes.iter().map(crate::RecipeEntry::asset);
            let plugins = index
                .plugins
                .iter()
                .flat_map(|p| p.blobs().map(|(asset, _)| asset));
            keep_blobs.extend(
                std::iter::once(index.detect_asset())
                    .chain(all)
                    .chain(plugins)
                    .map(|a| OsString::from(a.file_name())),
            );
        }
        let indexes = self.layout.indexes();
        for entry in self.fs.list(&indexes)? {
            if !keep_dirs.contains(&entry.name) {
                self.fs
                    .remove_tree(&indexes.join(&entry.name), &self.layout.root)?;
            }
        }
        let blobs = self.layout.blobs();
        for entry in self.fs.list(&blobs)? {
            if !keep_blobs.contains(&entry.name) {
                self.fs.remove_file(&blobs.join(&entry.name))?;
            }
        }
        Ok(())
    }
}
