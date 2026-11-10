//! Plugin items: the selected items whose *local* recipe item names a
//! plugin (the archive cannot ask for one), the consent blockers and the
//! plan's [`PlannedPlugin`] list.

use carrova_plugin::ContextField;
use carrova_recipe::HandlerRef;

use super::Planner;
use super::skip::SkipReason;
use crate::plan::{Blocker, PlannedPlugin};

impl Planner<'_> {
    /// Record the plugin of every selected plugin item: available here →
    /// [`Self::plugin_items`]; not available, or needing a sender home the
    /// archive lacks → [`Self::plugin_skips`] (its entries are skipped). Each plugin in use the user has not
    /// allowed is a [`Blocker::UnconfirmedPlugin`].
    pub(super) fn resolve_plugins(&mut self) {
        let manifest = self.manifest;
        for idx in self.selected.clone() {
            let Some(HandlerRef::Plugin(id)) = self
                .local_item(&manifest.items[idx])
                .and_then(|i| i.handler.as_ref())
            else {
                continue;
            };
            let key = self.keys[idx].clone();
            match self.plugins.iter().find(|p| p.manifest().id == *id) {
                Some(plugin)
                    if self.manifest.source_home.is_none()
                        && plugin
                            .manifest()
                            .context
                            .contains(&ContextField::SourceHome) =>
                {
                    self.plugin_skips
                        .insert(key, SkipReason::NoSourceHome(id.clone()));
                }
                Some(plugin) => {
                    self.plugin_items.insert(key, plugin);
                }
                None => {
                    self.plugin_skips
                        .insert(key, SkipReason::MissingPlugin(id.clone()));
                }
            }
        }
        for planned in self.planned_plugins() {
            if !self.options.confirmed_plugins.contains(&planned.plugin) {
                self.blockers.push(Blocker::UnconfirmedPlugin {
                    plugin: planned.plugin,
                    items: planned.items,
                });
            }
        }
    }

    /// One entry per plugin in use, in order of first use; items in
    /// manifest order.
    pub(super) fn planned_plugins(&self) -> Vec<PlannedPlugin> {
        let mut out = Vec::new();
        for key in self.selected.iter().map(|&i| &self.keys[i]) {
            if let Some(plugin) = self.plugin_items.get(key) {
                PlannedPlugin::add(&mut out, plugin.manifest(), key.clone());
            }
        }
        out
    }
}
