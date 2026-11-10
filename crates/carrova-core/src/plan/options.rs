//! The user's decisions for an import ([`PlanOptions`]). Defaults are
//! suggestions only.

use std::collections::BTreeMap;

use carrova_platform::SemanticRoot;
use serde::{Deserialize, Serialize};

use crate::key::ItemKey;

/// What to do when a target already exists (and differs).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ConflictRule {
    /// Suggested: move the existing file to the rollback store, write incoming.
    #[default]
    Replace,
    /// Keep existing; write incoming next to it under the keep-both name.
    KeepBoth,
    /// Keep existing; don't write incoming.
    Skip,
}

/// Restore an item somewhere else: swap its root and/or prefix its paths.
/// The result is confined exactly like any other target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Remap {
    /// Replace the entry's root (None = keep).
    pub root: Option<SemanticRoot>,
    /// Relative prefix prepended to the entry's rel path (validated RelPath).
    pub rel_prefix: Option<String>,
}

/// How long rollback points are kept.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Retention {
    /// Suggested: until the user dismisses it.
    #[default]
    UntilDismissed,
    /// Discard right after a successful apply.
    DiscardOnSuccess,
}

/// Every user decision for an import. Defaults are suggestions only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlanOptions {
    /// Archive item keys to import; `None` = every item the receiver does
    /// not treat as sensitive ([`super::Plan::sensitive`]) plus every
    /// confirmed one (see [`Self::picks`]).
    pub items: Option<Vec<ItemKey>>,
    /// Sensitive item keys the user explicitly confirmed (required to import them).
    pub confirmed_sensitive: Vec<ItemKey>,
    /// Rule when nothing more specific is set (suggested: Replace).
    pub default_rule: ConflictRule,
    /// Per-item rule (item key → rule).
    pub item_rules: BTreeMap<ItemKey, ConflictRule>,
    /// Per-file rule ([`RootedPath`](crate::key::RootedPath) text of the
    /// *target* → rule). Wins over item rules.
    pub path_rules: BTreeMap<String, ConflictRule>,
    /// Per-item remap.
    pub remaps: BTreeMap<ItemKey, Remap>,
    /// Apply even if an app that must be closed is running.
    pub ignore_requires_closed: bool,
    /// Apply even if free space looks insufficient.
    pub ignore_free_space: bool,
    /// Keep-both suffix template; `{date}` → `YYYYMMDD`. Suggested `carrova-{date}`.
    pub keep_both_suffix: String,
    pub retention: Retention,
    /// Opt-in: let handlers import trust settings (gpg ownertrust). Off by default.
    #[serde(default)]
    pub import_trust: bool,
    /// Opt-in: keep group-write bits from the archive (e.g. 0664 under a
    /// umask-002 setup). World-write, setuid and setgid are never restored.
    #[serde(default)]
    pub keep_group_write: bool,
}

impl PlanOptions {
    /// Whether these options pick item `key` for import — the one rule the
    /// planner, the checklist and every front-end share. `sensitive` is
    /// why the item needs explicit confirmation (`None`: it does not). With
    /// an explicit [`Self::items`] list: exactly the listed items. Without
    /// one: every item except a sensitive one the user has not confirmed
    /// ([`Self::is_confirmed`]).
    #[must_use]
    pub fn picks(&self, key: &ItemKey, sensitive: Option<&str>) -> bool {
        match &self.items {
            Some(list) => list.contains(key),
            None => !self.awaits_confirmation(key, sensitive),
        }
    }

    /// Whether item `key` is sensitive (`sensitive` holds why) and the user
    /// has not confirmed it yet: importing it needs that confirmation.
    #[must_use]
    pub fn awaits_confirmation(&self, key: &ItemKey, sensitive: Option<&str>) -> bool {
        sensitive.is_some() && !self.is_confirmed(key)
    }

    /// Whether the user explicitly confirmed importing sensitive item `key`.
    #[must_use]
    pub fn is_confirmed(&self, key: &ItemKey) -> bool {
        self.confirmed_sensitive.contains(key)
    }

    /// Conflict rule of item `item` when no per-file rule applies: its
    /// per-item rule, else the default.
    #[must_use]
    pub(crate) fn item_rule(&self, item: &ItemKey) -> ConflictRule {
        self.item_rules
            .get(item)
            .copied()
            .unwrap_or(self.default_rule)
    }
}

impl Default for PlanOptions {
    fn default() -> Self {
        Self {
            items: None,
            confirmed_sensitive: Vec::new(),
            default_rule: ConflictRule::Replace,
            item_rules: BTreeMap::new(),
            path_rules: BTreeMap::new(),
            remaps: BTreeMap::new(),
            ignore_requires_closed: false,
            ignore_free_space: false,
            keep_both_suffix: "carrova-{date}".into(),
            retention: Retention::UntilDismissed,
            import_trust: false,
            keep_group_write: false,
        }
    }
}

#[cfg(test)]
#[path = "../../tests/plan/options/unit.rs"]
mod tests;
