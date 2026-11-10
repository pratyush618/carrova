//! [`Planner`]: computes a [`Plan`] from a manifest, the local recipes and
//! the user's options. Reads the receiver's filesystem, never writes.
//!
//! Passes, in order (warnings keep this order): select items and classify
//! sensitivity ([`sensitivity`]), resolve every selected entry's final
//! target ([`targets`]), decide each target's action ([`decide`]), then
//! the item-level checks — handlers, apps, free space ([`checks`]).
//! Plugin items are resolved right after selection ([`plugins`]).

mod checks;
mod cross_os;
mod decide;
mod os_rules;
mod plugins;
mod roots;
mod sensitivity;
mod skip;
mod targets;

use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use carrova_manifest::{Manifest, RelPath};
use carrova_platform::Platform;
use carrova_plugin::Plugin;
use carrova_recipe::Recipe;

use super::confine::{Probe, probe};
use super::{Blocker, Plan, PlanOptions, PlanWarning, keep_both_suffix};
use crate::error::{CoreError, Result};
use crate::key::ItemKey;
use crate::mode::ModePolicy;
use crate::session::SessionId;
use crate::stage::StageLayout;
use cross_os::Rebased;
use roots::Roots;
use skip::SkipReason;

/// Planner state for one [`super::plan_on_date`] call.
pub(super) struct Planner<'a> {
    platform: &'a dyn Platform,
    recipes: &'a [Recipe],
    plugins: &'a [Plugin],
    manifest: &'a Manifest,
    options: &'a PlanOptions,
    policy: ModePolicy,
    /// Keep-both suffix with the date filled in.
    suffix: String,
    /// Selected item indices, manifest order.
    selected: Vec<usize>,
    /// Item index → key.
    keys: Vec<ItemKey>,
    /// Validated remap prefixes (item key → prefix).
    prefixes: HashMap<ItemKey, RelPath>,
    roots: Roots,
    probes: RefCell<HashMap<PathBuf, Probe>>,
    blockers: Vec<Blocker>,
    warnings: Vec<PlanWarning>,
    /// Receiver-side sensitive items (key → reason).
    sensitive: BTreeMap<ItemKey, String>,
    /// Cross-OS items rebased onto this OS's slots (foreign archives).
    rebased: HashMap<ItemKey, Rebased>,
    /// Cross-OS items that cannot be rebased (key → why).
    unmapped: HashMap<ItemKey, String>,
    /// Selected items whose files pass through a plugin.
    plugin_items: HashMap<ItemKey, &'a Plugin>,
    /// Selected plugin items that cannot run here, and why (skipped).
    plugin_skips: HashMap<ItemKey, SkipReason>,
}

impl<'a> Planner<'a> {
    /// Validate the options against the manifest, inspect the roots and
    /// select the items.
    pub(super) fn new(
        platform: &'a dyn Platform,
        recipes: &'a [Recipe],
        plugins: &'a [Plugin],
        manifest: &'a Manifest,
        options: &'a PlanOptions,
        date: &str,
    ) -> Result<Self> {
        let keys = unique_item_keys(manifest)?;
        check_option_keys(options, &keys)?;
        let mut planner = Self {
            platform,
            recipes,
            plugins,
            manifest,
            options,
            policy: ModePolicy::of(options),
            suffix: keep_both_suffix(&options.keep_both_suffix, date)?,
            selected: Vec::new(),
            keys,
            prefixes: remap_prefixes(options)?,
            roots: Roots::inspect(platform),
            probes: RefCell::new(HashMap::new()),
            blockers: Vec::new(),
            warnings: Vec::new(),
            sensitive: BTreeMap::new(),
            rebased: HashMap::new(),
            unmapped: HashMap::new(),
            plugin_items: HashMap::new(),
            plugin_skips: HashMap::new(),
        };
        planner.rebase_items();
        planner.select();
        planner.resolve_plugins();
        Ok(planner)
    }

    /// Run every pass and assemble the plan.
    pub(super) fn run(mut self, session: &SessionId) -> Result<Plan> {
        let targets = self.targets()?;
        let actions = self.actions(&targets);
        let handlers = self.handlers();
        let missing_apps = self.missing_apps();
        let running_apps = self.running_apps();
        let free_space = self.free_space();
        let app_names = self.app_names(missing_apps.iter().chain(&running_apps));
        let mut plan = Plan {
            session: session.clone(),
            options: self.options.clone(),
            actions,
            handlers,
            bytes_to_write: 0,
            free_space,
            missing_apps,
            running_apps,
            app_names,
            mappings: self.mappings(),
            plugins: self.planned_plugins(),
            blockers: Vec::new(),
            warnings: Vec::new(),
            sensitive: std::mem::take(&mut self.sensitive),
        };
        // Space: exactly what staging will write (one source of truth).
        let layout = StageLayout::new(&plan, self.manifest)?;
        plan.bytes_to_write = layout.total_bytes();
        self.check_space(&crate::space::needs(self.platform, &plan, &layout));
        plan.blockers = self.blockers;
        plan.warnings = self.warnings;
        Ok(plan)
    }

    fn warn(&mut self, item: Option<&ItemKey>, message: String) {
        self.warnings.push(PlanWarning {
            item: item.cloned(),
            message,
        });
    }

    /// lstat with cache. `Ok(None)` = absent; `Err` = cannot inspect.
    fn probe(&self, path: &Path) -> Probe {
        if let Some(p) = self.probes.borrow().get(path) {
            return p.clone();
        }
        let p = probe(self.platform, path);
        self.probes
            .borrow_mut()
            .insert(path.to_path_buf(), p.clone());
        p
    }

    fn recipe(&self, id: &str) -> Option<&'a Recipe> {
        self.recipes.iter().find(|r| r.id == id)
    }
}

/// Item keys in manifest order; an archive listing a key twice is refused.
fn unique_item_keys(manifest: &Manifest) -> Result<Vec<ItemKey>> {
    let keys: Vec<ItemKey> = manifest.items.iter().map(ItemKey::of).collect();
    let mut known = HashSet::new();
    match keys.iter().find(|k| !known.insert(*k)) {
        Some(k) => Err(CoreError::Selection(format!(
            "archive lists item `{k}` twice"
        ))),
        None => Ok(keys),
    }
}

/// Every item key the options name must exist in the archive.
fn check_option_keys(options: &PlanOptions, keys: &[ItemKey]) -> Result<()> {
    let known: HashSet<&ItemKey> = keys.iter().collect();
    let lists: [(&str, Vec<&ItemKey>); 5] = [
        ("items", options.items.iter().flatten().collect()),
        (
            "confirmed_sensitive",
            options.confirmed_sensitive.iter().collect(),
        ),
        ("item_rules", options.item_rules.keys().collect()),
        ("remaps", options.remaps.keys().collect()),
        ("allow_cross_os", options.allow_cross_os.iter().collect()),
    ];
    for (what, list) in lists {
        if let Some(k) = list.into_iter().find(|k| !known.contains(k)) {
            return Err(CoreError::Selection(format!(
                "unknown item `{k}` in {what}"
            )));
        }
    }
    Ok(())
}

/// Remap prefixes validated as relative paths (item key → prefix).
fn remap_prefixes(options: &PlanOptions) -> Result<HashMap<ItemKey, RelPath>> {
    let mut prefixes = HashMap::new();
    for (key, r) in &options.remaps {
        if let Some(p) = r.rel_prefix.as_deref() {
            let p = RelPath::new(p).map_err(|e| CoreError::UnsafePath {
                path: p.to_owned(),
                reason: format!("remap prefix for `{key}`: {e}"),
            })?;
            prefixes.insert(key.clone(), p);
        }
    }
    Ok(prefixes)
}
