//! [`SelectionEdit`]: the one way front-ends change a [`Selection`], and
//! the per-item primitives it is made of.

use carrova_recipe::PluginId;
use serde::{Deserialize, Serialize};

use super::{ItemChoice, Preset, Selection, SelectionIssue};
use crate::key::{ItemKey, ItemTarget};

/// One change the user makes to what moves ([`Selection::apply`]). Each
/// is a plain change to the [`Selection`]; whether the result is usable
/// is [`Selection::issues`]' call. Unrecognised directories are items
/// too (`raw:` keys).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SelectionEdit {
    /// Re-apply a preset to every tick (confirmations and exclusions stay).
    Preset {
        /// The pre-fill.
        preset: Preset,
    },
    /// An app's row: ticks or unticks its ordinary items (never sensitive
    /// or unmovable ones).
    App {
        /// Recipe id.
        app: String,
        /// Tick (`true`) or untick.
        included: bool,
    },
    /// Tick or untick one item.
    Item {
        /// The item.
        item: ItemKey,
        /// Tick (`true`) or untick.
        included: bool,
    },
    /// Tick or untick one path or file inside an item (`"<root>:<rel>"`;
    /// unticking a directory unticks everything below it).
    Path {
        /// The item.
        item: ItemKey,
        /// Path key under the item.
        path: String,
        /// Re-tick (`true`) or untick.
        included: bool,
    },
    /// The user's explicit answer for moving a sensitive item.
    ConfirmSensitive {
        /// The item.
        item: ItemKey,
        /// Confirmed (`true`) or confirmation withdrawn.
        confirmed: bool,
    },
    /// Override an item's unmovable mark (after seeing its reason).
    ForceUnmovable {
        /// The item.
        item: ItemKey,
    },
    /// Tick or untick every movable, non-sensitive item under the cache
    /// root.
    Caches {
        /// Tick (`true`) or untick.
        included: bool,
    },
    /// The user's answer for an exporting plugin
    /// ([`Selection::confirmed_plugins`]), after seeing what it may do.
    AllowPlugin {
        /// Plugin id.
        #[cfg_attr(feature = "ts", ts(as = "String"))]
        plugin: PluginId,
        /// Allowed (`true`) or withdrawn.
        allow: bool,
    },
}

impl Selection {
    /// Apply one user edit. An edit naming an unknown item is a
    /// [`SelectionIssue`] and changes nothing.
    ///
    /// # Errors
    ///
    /// The edit names an item that is not in the selection.
    pub fn apply(&mut self, edit: SelectionEdit) -> Result<(), SelectionIssue> {
        match edit {
            SelectionEdit::Preset { preset } => self.apply_preset(preset),
            SelectionEdit::App { app, included } => {
                let of_app = |key: &ItemKey| matches!(key.target(), Some(ItemTarget::Recipe { recipe, .. }) if recipe == app);
                for (_, c) in self
                    .items
                    .iter_mut()
                    .filter(|(key, c)| of_app(key) && c.is_ordinary())
                {
                    c.included = included;
                }
            }
            SelectionEdit::Item { item, included } => self.set_included(item.as_str(), included)?,
            SelectionEdit::Path {
                item,
                path,
                included,
            } => self.set_path_excluded(item.as_str(), &path, !included)?,
            SelectionEdit::ConfirmSensitive { item, confirmed } => {
                self.choice(item.as_str())?.confirmed_sensitive = confirmed;
            }
            SelectionEdit::ForceUnmovable { item } => self.force_unmovable(item.as_str())?,
            SelectionEdit::Caches { included } => {
                for c in self
                    .items
                    .values_mut()
                    .filter(|c| c.is_cache() && c.is_movable() && !c.sensitive)
                {
                    c.included = included;
                }
            }
            SelectionEdit::AllowPlugin { plugin, allow } => {
                let list = &mut self.confirmed_plugins;
                list.retain(|p| *p != plugin);
                if allow {
                    list.push(plugin);
                }
            }
        }
        Ok(())
    }

    /// Apply `edits` in order, stopping at the first issue.
    ///
    /// # Errors
    ///
    /// The first edit naming an item that is not in the selection.
    pub fn apply_all(
        &mut self,
        edits: impl IntoIterator<Item = SelectionEdit>,
    ) -> Result<(), SelectionIssue> {
        edits.into_iter().try_for_each(|edit| self.apply(edit))
    }

    pub(super) fn choice(&mut self, item: &str) -> Result<&mut ItemChoice, SelectionIssue> {
        self.items.get_mut(item).ok_or_else(|| SelectionIssue {
            item: item.into(),
            problem: format!("unknown item `{item}`"),
            plugin: None,
        })
    }

    /// Tick/untick an item ([`SelectionEdit::Item`]).
    ///
    /// # Errors
    ///
    /// Unknown item.
    pub fn set_included(&mut self, item: &str, included: bool) -> Result<(), SelectionIssue> {
        self.choice(item)?.included = included;
        Ok(())
    }

    /// Untick (`excluded = true`) or re-tick one path/file under an item
    /// ([`SelectionEdit::Path`]). Re-ticking removes exactly that key (a
    /// file below an excluded directory stays excluded until the
    /// directory is re-ticked).
    ///
    /// # Errors
    ///
    /// Unknown item.
    pub fn set_path_excluded(
        &mut self,
        item: &str,
        path: &str,
        excluded: bool,
    ) -> Result<(), SelectionIssue> {
        let c = self.choice(item)?;
        let pos = c.excluded_paths.binary_search_by(|p| p.as_str().cmp(path));
        match (excluded, pos) {
            (true, Err(i)) => c.excluded_paths.insert(i, path.to_owned()),
            (false, Ok(i)) => {
                c.excluded_paths.remove(i);
            }
            _ => {}
        }
        Ok(())
    }

    /// Record the explicit confirmation for a sensitive item
    /// ([`SelectionEdit::ConfirmSensitive`]).
    ///
    /// # Errors
    ///
    /// Unknown item.
    pub fn confirm_sensitive(&mut self, item: &str) -> Result<(), SelectionIssue> {
        self.choice(item)?.confirmed_sensitive = true;
        Ok(())
    }

    /// Override an item's unmovable mark ([`SelectionEdit::ForceUnmovable`]).
    ///
    /// # Errors
    ///
    /// Unknown item.
    pub fn force_unmovable(&mut self, item: &str) -> Result<(), SelectionIssue> {
        self.choice(item)?.force_unmovable = true;
        Ok(())
    }
}
