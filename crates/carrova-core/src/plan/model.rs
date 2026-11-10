//! The plan the user approves: [`Plan`] and its parts.

use std::collections::BTreeMap;
use std::path::PathBuf;

use carrova_manifest::ManifestItem;
use carrova_platform::SemanticRoot;
pub use carrova_recipe::Handler;
use carrova_recipe::PluginId;
use serde::{Deserialize, Serialize};

use super::{Blocker, PlanOptions};
use crate::error::{CoreError, Result};
use crate::key::ItemKey;
use crate::session::SessionId;

/// [`Plan::sensitivity`] of an item only the source flags as sensitive.
pub const SOURCE_SENSITIVE: &str = "marked sensitive by the source";

/// Why item `item` of recipe `recipe` needs explicit confirmation when
/// the recipe flags it sensitive: the one text the sender's scan and the
/// receiver's plan both show.
#[must_use]
pub fn recipe_sensitive(recipe: &str, item: &str) -> String {
    format!("marked sensitive by recipe {recipe}/{item}")
}

/// Kind of object an action writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ObjectKind {
    File,
    Dir,
    Symlink,
}

/// What happens to one target.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum Action {
    /// Target absent: create.
    Create,
    /// Target exists and differs: back up existing, write incoming.
    Replace,
    /// Target exists: write incoming at `rel` instead (same root).
    KeepBoth { rel: String },
    /// Not written.
    Skip { reason: String },
    /// Target already identical (same kind, hash/target, mode): nothing to do.
    Identical,
}

impl Action {
    /// Stable name (the serde tag), e.g. `keep-both`.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Replace => "replace",
            Self::KeepBoth { .. } => "keep-both",
            Self::Skip { .. } => "skip",
            Self::Identical => "identical",
        }
    }

    /// `Skip` with `reason`.
    #[must_use]
    pub fn skip(reason: impl Into<String>) -> Self {
        Self::Skip {
            reason: reason.into(),
        }
    }

    /// Nothing is written (`Skip` / `Identical`).
    #[must_use]
    pub const fn is_unchanged(&self) -> bool {
        matches!(self, Self::Skip { .. } | Self::Identical)
    }
}

/// One planned filesystem change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlannedAction {
    /// Archive item key.
    pub item: ItemKey,
    /// Index of the entry within that manifest item.
    pub entry_index: usize,
    pub kind: ObjectKind,
    /// Final target (after remap).
    pub root: SemanticRoot,
    pub rel: String,
    /// Resolved absolute target on this machine.
    pub abs: PathBuf,
    /// Bytes to write (files).
    pub size: u64,
    /// Final permission bits apply sets: the manifest mode under
    /// `ModePolicy` (for an existing directory, never looser than the
    /// existing mode).
    pub mode: Option<u32>,
    /// Symlink actions: the exact (already vetted) target string to write
    /// verbatim. `None` for files and directories.
    pub link_target: Option<String>,
    /// Modification time from the manifest entry (ns since the epoch).
    pub mtime_ns: Option<i64>,
    pub action: Action,
}

impl PlannedAction {
    /// Relative path actually written: the keep-both name, else `rel`.
    #[must_use]
    pub fn target_rel(&self) -> &str {
        match &self.action {
            Action::KeepBoth { rel } => rel,
            _ => &self.rel,
        }
    }
}

/// A built-in handler run for an item; `backup` lists the files it mutates
/// (backed up before it runs, restored on rollback).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlannedHandler {
    pub item: ItemKey,
    #[cfg_attr(feature = "ts", ts(as = "String"))]
    pub handler: Handler,
    pub backup: Vec<(SemanticRoot, String)>,
    /// Also import trust settings (gpg ownertrust). Off unless the user
    /// opted in via [`PlanOptions::import_trust`]: archive trust data could
    /// mark an attacker's key as trusted.
    pub import_trust: bool,
}

/// A plugin the plan runs over the files of `items`, with what the user
/// is asked to allow ([`PlanOptions::confirmed_plugins`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlannedPlugin {
    #[cfg_attr(feature = "ts", ts(as = "String"))]
    pub plugin: PluginId,
    pub version: String,
    pub description: String,
    /// Plain sentences, one per permission (the plugin's manifest).
    pub permissions: Vec<String>,
    /// Selected items whose files it changes, manifest order.
    pub items: Vec<ItemKey>,
}

/// One slot of a cross-OS item restored at this computer's path for it:
/// the source OS's path `from` (from the archive) becomes the local
/// recipe's path `to` for the same slot; every entry below `from` lands
/// below `to`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PathMapping {
    pub item: ItemKey,
    pub from: (SemanticRoot, String),
    pub to: (SemanticRoot, String),
    /// `to` resolved on this computer; `None` if its root is unusable here.
    pub to_abs: Option<PathBuf>,
}

/// Non-fatal note shown with the plan (overrides used, older app, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlanWarning {
    pub item: Option<ItemKey>,
    pub message: String,
}

/// The dry-run result the user approves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Plan {
    pub session: SessionId,
    pub options: PlanOptions,
    pub actions: Vec<PlannedAction>,
    pub handlers: Vec<PlannedHandler>,
    /// Bytes that will be written.
    pub bytes_to_write: u64,
    /// Free space at the home filesystem, if known.
    pub free_space: Option<u64>,
    /// Apps (recipe ids) whose data is imported but which aren't detected here.
    pub missing_apps: Vec<String>,
    /// Apps that must be closed and are running.
    pub running_apps: Vec<String>,
    /// Display names of the apps in [`Self::missing_apps`] and
    /// [`Self::running_apps`] that this machine has a recipe for (recipe
    /// id → name). Empty in plans written before names were recorded.
    #[serde(default)]
    pub app_names: BTreeMap<String, String>,
    /// Cross-OS items' path mappings, item and slot order (empty when the
    /// archive comes from this OS family).
    #[serde(default)]
    pub mappings: Vec<PathMapping>,
    /// Plugins run over selected items' files, by id (whether allowed or
    /// not yet).
    #[serde(default)]
    pub plugins: Vec<PlannedPlugin>,
    /// Blockers not overridden by the user; apply refuses while non-empty.
    #[serde(deserialize_with = "super::blocker::read_blockers")]
    pub blockers: Vec<Blocker>,
    pub warnings: Vec<PlanWarning>,
    /// Items the *receiver* treats as sensitive (needing explicit
    /// confirmation), with the reason — computed locally, never trusted
    /// from the archive alone. Item key → reason.
    #[serde(default)]
    pub sensitive: BTreeMap<ItemKey, String>,
}

impl Plan {
    /// Why manifest item `item` needs explicit confirmation, `None` if it
    /// does not: the receiver's own judgement ([`Self::sensitive`]), else
    /// [`SOURCE_SENSITIVE`] when only the source flags it. Nothing is ever
    /// treated as less sensitive than either side says.
    #[must_use]
    pub fn sensitivity<'a>(&'a self, item: &ManifestItem) -> Option<&'a str> {
        self.sensitive
            .get(&ItemKey::of(item))
            .map(String::as_str)
            .or_else(|| item.flags.sensitive.then_some(SOURCE_SENSITIVE))
    }

    /// Whether this plan imports manifest item `item`
    /// ([`PlanOptions::picks`] with [`Self::sensitivity`]).
    #[must_use]
    pub fn picks(&self, item: &ManifestItem) -> bool {
        self.options
            .picks(&ItemKey::of(item), self.sensitivity(item))
    }

    /// Display name of app `id` ([`Self::app_names`], else the id).
    #[must_use]
    pub fn app_name<'a>(&'a self, id: &'a str) -> &'a str {
        self.app_names.get(id).map_or(id, String::as_str)
    }

    /// Targets that already exist and differ (Replace / KeepBoth / Skip-by-rule).
    pub fn conflicts(&self) -> impl Iterator<Item = &PlannedAction> {
        self.actions
            .iter()
            .filter(|a| !matches!(a.action, Action::Create | Action::Identical))
    }

    /// [`CoreError::PlanBlocked`] (every blocker, `; `-separated) unless
    /// nothing blocks the plan.
    pub fn ensure_unblocked(&self) -> Result<()> {
        if self.blockers.is_empty() {
            return Ok(());
        }
        let all: Vec<String> = self.blockers.iter().map(ToString::to_string).collect();
        Err(CoreError::PlanBlocked(all.join("; ")))
    }
}
