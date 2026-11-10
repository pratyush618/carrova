//! Scan: what is on this machine and what could move.
//!
//! Pipeline: **detect → resolve → size → unrecognised**.
//!
//! 1. Every recipe is checked with [`carrova_recipe::detect`].
//! 2. Items of *detected* apps are resolved with
//!    [`carrova_recipe::resolve_item`]; sizes and file counts come from the
//!    resolved entries (regular files only). Nothing is hashed or chunked.
//! 3. Items with `requires_closed` are marked when the app looks running
//!    (see [`AppReport::running`]).
//! 4. Unrecognised data: each immediate child **directory** of `user_config`
//!    and `user_data` that no detected recipe item claims is reported as a
//!    [`RawItem`] with its total size and file count. Top-level files and
//!    symlinks are not reported (single files there are usually shared
//!    desktop state, and symlinks are never followed). Raw items are never
//!    selected by default.
//!
//! A [`Cancel`] (via [`crate::Exporter::start_with`]) is checked before
//! every recipe and every entry walked, so even a huge unrecognised tree
//! stops promptly.
//!
//! The scan is read-only and never fails because of a single unreadable path:
//! problems are recorded per item (`skipped`), per raw dir (`unreadable`) or
//! in [`ScanReport::warnings`]. All output is deterministically ordered.

mod apps;
mod machine;
mod unrecognised;

use std::collections::BTreeMap;

use carrova_platform::{OsKind, Platform, PlatformError, SemanticRoot};
use carrova_recipe::Recipe;
use serde::{Deserialize, Serialize};
use tracing::{debug, debug_span};

use crate::cancel::Cancel;

/// What to include in a scan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScanOptions {
    /// Also list apps whose recipe did not detect them (without items).
    pub include_undetected: bool,
    /// Look for unrecognised directories under `user_config` / `user_data`.
    pub unrecognised: bool,
}

impl Default for ScanOptions {
    fn default() -> Self {
        Self {
            include_undetected: false,
            unrecognised: true,
        }
    }
}

/// Fatal scan failure. Everything else degrades into warnings.
#[derive(Debug, thiserror::Error)]
pub enum ScanError {
    /// Without a home directory there is nothing to scan.
    #[error("cannot resolve the home directory: {0}")]
    HomeUnavailable(#[source] PlatformError),
    /// The scan's [`Cancel`] fired.
    #[error("the scan was cancelled")]
    Cancelled,
}

impl ScanError {
    /// [`Self::Cancelled`] once `cancel` fired.
    fn check(cancel: &Cancel) -> Result<(), Self> {
        if cancel.is_cancelled() {
            return Err(Self::Cancelled);
        }
        Ok(())
    }
}

/// Result of [`scan`]. Serialises to the `carrova scan --json` shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScanReport {
    pub platform: PlatformReport,
    /// Sorted by `(category, id)`.
    pub apps: Vec<AppReport>,
    /// Sorted by `(root, rel)`.
    pub unrecognised: Vec<RawItem>,
    pub totals: Totals,
    /// Non-fatal problems (unavailable roots, failed process listing, ...).
    pub warnings: Vec<String>,
}

/// Facts about the machine being scanned.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlatformReport {
    /// Serialised as [`OsKind::as_str`] (`linux`, `macos`, `windows`).
    #[serde(serialize_with = "os_id")]
    #[cfg_attr(feature = "ts", ts(as = "String"))]
    pub os: OsKind,
    /// Every semantic root that resolved, by root.
    pub roots: BTreeMap<SemanticRoot, String>,
    /// Bytes free on the filesystem holding `home`, if known.
    pub free_space: Option<u64>,
    /// Snapshot support of the filesystem holding `home`, if known.
    pub snapshot: Option<Snapshot>,
}

fn os_id<S: serde::Serializer>(os: &OsKind, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(os.as_str())
}

/// Snapshot support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Snapshot {
    None,
    Btrfs,
    Lvm,
}

/// One recipe's outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AppReport {
    pub id: String,
    pub name: String,
    pub category: String,
    pub detected: bool,
    /// Human-readable reasons the app counts as present, in recipe order.
    pub evidence: Vec<String>,
    /// Best effort: a running process's name or executable file name equals
    /// one of the recipe's `binary` detect rules or its `processes` (daemons) (see
    /// `ProcessInfo::matches_binary`, which handles OS name truncation). Only checked when a detected app has a
    /// movable `requires_closed` item; `false` if process listing failed.
    pub running: bool,
    /// Recipe order. Empty for undetected apps.
    pub items: Vec<ItemReport>,
}

/// One recipe item as found on this machine.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ItemReport {
    pub id: String,
    pub label: String,
    /// Bytes in regular files.
    pub size: u64,
    /// Number of regular files.
    pub file_count: u64,
    pub flags: ItemFlags,
    /// Why the item needs explicit confirmation before it moves (the
    /// recipe flags it sensitive); `None` if it does not.
    pub sensitive_reason: Option<String>,
    /// Declared recipe paths, as `root:rel`, in recipe order.
    pub paths: Vec<String>,
    /// Why the item can never move (resolves to nothing).
    pub unmovable: Option<String>,
    /// Recipe paths that do not exist, as `root:rel`.
    pub missing: Vec<String>,
    /// Paths left out of the item, with reasons.
    pub skipped: Vec<SkippedPath>,
    /// `requires_closed` and the app is currently running.
    pub app_running: bool,
}

/// Item flags, with the effective default selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ItemFlags {
    pub sensitive: bool,
    pub requires_closed: bool,
    pub portability: ItemPortability,
    /// Pre-selected in the picker.
    pub default_selected: bool,
}

/// Where an item's data can be restored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ItemPortability {
    SameOs,
    CrossOs,
}

/// A path left out of an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SkippedPath {
    /// `root:rel`.
    pub path: String,
    pub reason: String,
}

/// A directory no recipe recognises. Never selected by default.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct RawItem {
    pub root: SemanticRoot,
    /// Directory name relative to `root` (lossy if not UTF-8).
    pub rel: String,
    /// Bytes in regular files (symlinks are not followed).
    pub size: u64,
    pub file_count: u64,
    /// Entries that could not be read (permission errors etc.).
    pub unreadable: u64,
}

/// Size totals.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Totals {
    /// Items selected by default.
    pub default_selected_size: u64,
    pub default_selected_files: u64,
    /// All items of detected apps.
    pub recognised_size: u64,
    /// All raw items.
    pub unrecognised_size: u64,
    /// `recognised_size + unrecognised_size`.
    pub total_size: u64,
}

/// Run a read-only scan of the current user's data.
pub fn scan(
    recipes: &[Recipe],
    platform: &dyn Platform,
    opts: ScanOptions,
) -> Result<ScanReport, ScanError> {
    scan_with(recipes, platform, opts, &Cancel::default())
}

/// [`scan`], ending with [`ScanError::Cancelled`] once `cancel` fires.
pub(crate) fn scan_with(
    recipes: &[Recipe],
    platform: &dyn Platform,
    opts: ScanOptions,
    cancel: &Cancel,
) -> Result<ScanReport, ScanError> {
    let _span = debug_span!("scan", recipes = recipes.len()).entered();
    let mut warnings = Vec::new();
    let machine = machine::survey(platform, &mut warnings)?;
    let recognised = apps::recognise(
        recipes,
        platform,
        &machine.roots,
        opts,
        cancel,
        &mut warnings,
    )?;
    let unrecognised = if opts.unrecognised {
        unrecognised::find(&machine.roots, &recognised.claims, cancel, &mut warnings)?
    } else {
        Vec::new()
    };
    let mut totals = recognised.totals;
    totals.unrecognised_size = unrecognised.iter().map(|r| r.size).sum();
    totals.total_size = totals.recognised_size + totals.unrecognised_size;

    debug!(
        apps = recognised.apps.len(),
        unrecognised = unrecognised.len(),
        warnings = warnings.len(),
        "scan done"
    );
    Ok(ScanReport {
        platform: machine.report,
        apps: recognised.apps,
        unrecognised,
        totals,
        warnings,
    })
}

#[cfg(test)]
#[path = "../../tests/scan/unit.rs"]
mod tests;
