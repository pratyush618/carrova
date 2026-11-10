//! Sender-side plugin export: each file of an item whose recipe handler is
//! a plugin declaring `export` goes through [`Plugin::export`] before it is
//! chunked, and the output is what is sent (kept in the capture, like a
//! handler artifact). The plugin runs only if the user allowed it on this
//! computer; otherwise the item is left out with the reason.

use std::path::Path;

use carrova_manifest::{DEFAULT_CHUNK_PARAMS, Entry, EntryKind, chunk_reader};
use carrova_platform::Platform;
use carrova_plugin::{Plugin, RunBudget};
use carrova_recipe::{HandlerRef, Item, PluginId};

use crate::error::{CoreError, Result};
use crate::key::RootedPath;
use crate::stage::read_capped;

/// One exporting plugin and what is left of its budget for this capture.
pub(super) struct ExportHook<'p> {
    plugin: &'p Plugin,
    budget: RunBudget,
}

/// The exporting plugins loaded here and the ones the user allowed.
pub(super) struct Exports<'p> {
    hooks: Vec<ExportHook<'p>>,
    allowed: &'p [PluginId],
}

impl<'p> Exports<'p> {
    pub(super) fn new(plugins: &'p [Plugin], allowed: &'p [PluginId]) -> Self {
        let hooks = plugins
            .iter()
            .filter(|p| p.manifest().export)
            .map(|plugin| ExportHook {
                plugin,
                budget: RunBudget::new(),
            })
            .collect();
        Self { hooks, allowed }
    }

    /// The hook `item`'s files go through: `None` unless its plugin is
    /// loaded here and exports; an error if the user did not allow it.
    pub(super) fn hook_for(&mut self, item: &Item) -> Result<Option<&mut ExportHook<'p>>> {
        let Some(HandlerRef::Plugin(id)) = &item.handler else {
            return Ok(None);
        };
        let Some(hook) = self
            .hooks
            .iter_mut()
            .find(|h| h.plugin.manifest().id == *id)
        else {
            return Ok(None);
        };
        if !self.allowed.contains(id) {
            return Err(CoreError::Plugin {
                plugin: id.to_string(),
                file: "this item".into(),
                reason: "it must run on this computer before sending, and was not allowed".into(),
            });
        }
        Ok(Some(hook))
    }
}

impl ExportHook<'_> {
    /// Export the captured file `entry` (at `abs`): its kind now describes
    /// the plugin's output, which is returned for the capture to keep.
    pub(super) fn apply(
        &mut self,
        platform: &dyn Platform,
        abs: &Path,
        entry: &mut Entry,
    ) -> Result<Vec<u8>> {
        let out = self
            .plugin
            .export(
                platform,
                entry.rel.as_str(),
                &read_capped(platform, abs)?,
                &mut self.budget,
            )
            .map_err(|e| CoreError::Plugin {
                plugin: self.plugin.manifest().id.to_string(),
                file: RootedPath::new(entry.root, entry.rel.as_str()).to_string(),
                reason: e.to_string(),
            })?;
        let fc = chunk_reader(out.as_slice(), DEFAULT_CHUNK_PARAMS)?;
        entry.kind = EntryKind::File {
            size: fc.size,
            hash: fc.file_hash,
            chunks: fc.chunks,
        };
        Ok(out)
    }
}
