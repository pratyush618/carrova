//! [`PlanEdit`]: the one way front-ends change [`PlanOptions`] while the
//! user reviews a plan ([`PlanOptions::apply_all`]).

use carrova_manifest::Manifest;
use carrova_recipe::PluginId;
use serde::{Deserialize, Serialize};

use super::{ConflictRule, Plan, PlanOptions, Remap, Retention};
use crate::error::CoreError;
use crate::key::ItemKey;

/// One change the user makes while reviewing a plan. Each is a plain
/// change to the [`PlanOptions`]; the next plan says what it means.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PlanEdit {
    /// The answer for a sensitive item: confirm and pick it, or leave it
    /// behind.
    AnswerSensitive {
        /// The item.
        item: ItemKey,
        /// Bring it (`true`) or leave it.
        bring: bool,
    },
    /// Leave every item of an app out (it is open and the user chose to
    /// skip it), or bring its items back.
    SkipApp {
        /// Recipe id.
        app: String,
        /// Skip (`true`) or bring back.
        skip: bool,
    },
    /// Restore a same-OS item from another OS anyway (picked again if it
    /// was left), or withdraw that.
    AllowCrossOs {
        /// The item.
        item: ItemKey,
        /// Allow (`true`) or withdraw.
        allow: bool,
    },
    /// Let a plugin change the files of the items that use it (the user
    /// saw its permissions), or withdraw that.
    AllowPlugin {
        /// The plugin.
        #[cfg_attr(feature = "ts", ts(as = "String"))]
        plugin: PluginId,
        /// Allow (`true`) or withdraw.
        allow: bool,
    },
    /// Clash rule for a whole item.
    ItemRule {
        /// The item.
        item: ItemKey,
        /// The rule.
        rule: ConflictRule,
    },
    /// Clash rule for one target path (`root:rel`); `None` follows the
    /// item's rule again.
    PathRule {
        /// Target path.
        path: String,
        /// The rule, or `None` to clear it.
        rule: Option<ConflictRule>,
    },
    /// Restore an item elsewhere (re-confined by the planner); `None` puts
    /// it back where it was.
    Remap {
        /// The item.
        item: ItemKey,
        /// Where to, or `None` to clear it.
        remap: Option<Remap>,
    },
    /// Keep-both suffix template (`{date}` → `YYYYMMDD`); an empty one
    /// keeps the current suffix.
    KeepBothSuffix {
        /// The template.
        suffix: String,
    },
    /// How long the undo point is kept.
    Retention {
        /// The retention.
        retention: Retention,
    },
    /// Let handlers import trust settings (gpg ownertrust).
    ImportTrust {
        /// Import them (`true`) or not.
        allow: bool,
    },
    /// Keep group-write bits from the source.
    KeepGroupWrite {
        /// Keep them (`true`) or not.
        keep: bool,
    },
    /// Override a plan-level check (confirmed by the user first).
    Override {
        /// Which check.
        check: PlanOverride,
    },
}

/// Plan-level checks the user may override ([`PlanEdit::Override`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PlanOverride {
    /// Import although apps that must be closed are running
    /// ([`PlanOptions::ignore_requires_closed`]).
    Running,
    /// Import although free space looks short
    /// ([`PlanOptions::ignore_free_space`]).
    Space,
}

/// An item the source offers, as [`PlanOptions::apply_all`] needs to
/// know it ([`Plan::offered`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offered {
    /// Item key.
    pub key: ItemKey,
    /// Recipe id of its app; `None` for an item without a recipe.
    pub app: Option<String>,
    /// The receiver treats it as sensitive ([`Plan::sensitivity`]).
    pub sensitive: bool,
    /// Plugins the plan runs over its files ([`Plan::plugins`]).
    pub plugins: Vec<PluginId>,
}

/// An edit that cannot apply; the options are unchanged.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanEditIssue {
    /// The edit names an item the source does not offer.
    #[error("unknown item `{0}`")]
    UnknownItem(ItemKey),
    /// The edit names a plugin the plan does not use.
    #[error("unknown plugin `{0}`")]
    UnknownPlugin(PluginId),
}

impl From<PlanEditIssue> for CoreError {
    fn from(issue: PlanEditIssue) -> Self {
        Self::Selection(issue.to_string())
    }
}

impl Plan {
    /// Every item of `manifest` with whether this plan treats it as
    /// sensitive, in manifest order.
    #[must_use]
    pub fn offered(&self, manifest: &Manifest) -> Vec<Offered> {
        manifest
            .items
            .iter()
            .map(|item| {
                let key = ItemKey::of(item);
                let plugins = self
                    .plugins
                    .iter()
                    .filter(|p| p.items.contains(&key))
                    .map(|p| p.plugin.clone())
                    .collect();
                Offered {
                    app: item.recipe_id.clone(),
                    sensitive: self.sensitivity(item).is_some(),
                    plugins,
                    key,
                }
            })
            .collect()
    }
}

impl PlanOptions {
    /// Apply one edit against the `offered` items. Toggles are set
    /// semantics: applying the same edit twice changes nothing more. An
    /// edit naming an unknown item or plugin changes nothing.
    ///
    /// # Errors
    ///
    /// The edit names an item the source does not offer, or a plugin the
    /// plan neither uses nor has allowed.
    pub fn apply(&mut self, edit: PlanEdit, offered: &[Offered]) -> Result<(), PlanEditIssue> {
        match edit {
            PlanEdit::AnswerSensitive { item, bring } => {
                known(&item, offered)?;
                if self.items.is_none() && !bring {
                    self.items = Some(self.picked(offered));
                }
                if let Some(items) = &mut self.items {
                    toggle(items, item.clone(), bring);
                }
                toggle(&mut self.confirmed_sensitive, item, bring);
            }
            PlanEdit::SkipApp { app, skip } => {
                let of_app = |o: &&Offered| o.app.as_deref() == Some(app.as_str());
                let mut items = self.picked(offered);
                items.retain(|k| !offered.iter().filter(of_app).any(|o| o.key == *k));
                if !skip {
                    items.extend(
                        offered
                            .iter()
                            .filter(of_app)
                            .filter(|o| !o.sensitive || self.is_confirmed(&o.key))
                            .map(|o| o.key.clone()),
                    );
                }
                self.items = Some(items);
            }
            PlanEdit::AllowCrossOs { item, allow } => {
                known(&item, offered)?;
                if let (true, Some(items)) = (allow, &mut self.items) {
                    toggle(items, item.clone(), true);
                }
                toggle(&mut self.allow_cross_os, item, allow);
            }
            PlanEdit::AllowPlugin { plugin, allow } => {
                let in_use = offered.iter().any(|o| o.plugins.contains(&plugin));
                if !in_use && !self.confirmed_plugins.contains(&plugin) {
                    return Err(PlanEditIssue::UnknownPlugin(plugin));
                }
                toggle(&mut self.confirmed_plugins, plugin, allow);
            }
            PlanEdit::ItemRule { item, rule } => {
                known(&item, offered)?;
                self.item_rules.insert(item, rule);
            }
            PlanEdit::PathRule { path, rule } => match rule {
                Some(rule) => drop(self.path_rules.insert(path, rule)),
                None => drop(self.path_rules.remove(&path)),
            },
            PlanEdit::Remap { item, remap } => {
                known(&item, offered)?;
                match remap {
                    Some(remap) => drop(self.remaps.insert(item, remap)),
                    None => drop(self.remaps.remove(&item)),
                }
            }
            PlanEdit::KeepBothSuffix { suffix } => {
                let suffix = suffix.trim();
                if !suffix.is_empty() {
                    suffix.clone_into(&mut self.keep_both_suffix);
                }
            }
            PlanEdit::Retention { retention } => self.retention = retention,
            PlanEdit::ImportTrust { allow } => self.import_trust = allow,
            PlanEdit::KeepGroupWrite { keep } => self.keep_group_write = keep,
            PlanEdit::Override { check } => match check {
                PlanOverride::Running => self.ignore_requires_closed = true,
                PlanOverride::Space => self.ignore_free_space = true,
            },
        }
        Ok(())
    }

    /// Apply `edits` in order against the `offered` items, stopping at the
    /// first issue.
    ///
    /// # Errors
    ///
    /// The first edit naming an unknown item or plugin.
    pub fn apply_all(
        &mut self,
        edits: impl IntoIterator<Item = PlanEdit>,
        offered: &[Offered],
    ) -> Result<(), PlanEditIssue> {
        edits
            .into_iter()
            .try_for_each(|edit| self.apply(edit, offered))
    }

    /// The offered items these options pick, as an explicit list.
    fn picked(&self, offered: &[Offered]) -> Vec<ItemKey> {
        offered
            .iter()
            .filter(|o| match &self.items {
                Some(list) => list.contains(&o.key),
                None => !o.sensitive || self.is_confirmed(&o.key),
            })
            .map(|o| o.key.clone())
            .collect()
    }
}

fn known(item: &ItemKey, offered: &[Offered]) -> Result<(), PlanEditIssue> {
    if offered.iter().any(|o| o.key == *item) {
        Ok(())
    } else {
        Err(PlanEditIssue::UnknownItem(item.clone()))
    }
}

/// Add `value` to `list` (once) or remove it.
fn toggle<T: PartialEq>(list: &mut Vec<T>, value: T, on: bool) {
    if !on {
        list.retain(|v| *v != value);
    } else if !list.contains(&value) {
        list.push(value);
    }
}

#[cfg(test)]
#[path = "../../tests/plan/edit/unit.rs"]
mod tests;
