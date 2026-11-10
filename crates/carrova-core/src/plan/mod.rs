//! Dry-run plan (receiver side). Pure: reads the receiver's filesystem
//! (lstat, hashes of existing files) but **never writes**. Nothing is
//! written before the user approves the plan.
//!
//! The user decides: which archive items to import, the conflict rule per
//! item and per file, target remaps, and whether to override the
//! requires-closed and free-space checks. Every override used appears in
//! [`Plan::warnings`]. Confinement is not overridable: every target is
//! re-normalised under its resolved root and walked with lstat; any symlink
//! component, or a symlink entry whose target escapes root+home, is refused,
//! and so is a symlink whose target goes through a link that already exists
//! on this machine.
//!
//! # Receiver-side trust
//! The archive may be crafted by an attacker (who may also hand over the
//! passphrase), so nothing in it decides what is safe. An item is
//! *sensitive* — off by default, importable only with explicit confirmation
//! — if any of these hold ([`Plan::sensitive`] records why):
//! - the archive flags it sensitive;
//! - it is not a known item of a local recipe (raw items without a
//!   recipe, unknown recipes or items): "unrecognised data from the archive";
//! - the local recipe item is flagged sensitive;
//! - one of its entries lies outside the local recipe item's declared paths;
//! - one of its final targets (after remap), or a file its handler changes,
//!   lies in a [`DANGEROUS_PATHS`] location (code that runs at login, SSH /
//!   GnuPG material, git command hooks, …).
//!
//! Modes from the archive follow the `mode` policy (never world-writable or
//! setuid/setgid, group-write only on opt-in, an existing directory is only
//! ever tightened); [`PlannedAction::mode`] is the final mode apply sets.
//! Bits not restored are summarised in one warning per item.

mod blocker;
mod confine;
mod dangerous;
mod model;
mod options;
mod planner;

use carrova_manifest::Manifest;
use carrova_platform::Platform;
use carrova_plugin::Plugin;
use carrova_recipe::Recipe;

use crate::error::{CoreError, Result};
use crate::session::SessionId;
use crate::time;

pub use crate::space::{
    FsLocation, SPACE_MARGIN_FIXED, SpaceNeed, SpaceShortfall, required_space, space_needs,
};
pub use blocker::Blocker;
pub use dangerous::{DANGEROUS_PATHS, DangerousPath};
pub use model::{
    Action, Handler, ObjectKind, PathMapping, Plan, PlanWarning, PlannedAction, PlannedHandler,
    PlannedPlugin, SOURCE_SENSITIVE, recipe_sensitive,
};
pub use options::{ConflictRule, PlanOptions, Remap, Retention};

/// Keep-both names tried per target (`<name>.<suffix>`, then `-2` … `-N`).
pub const KEEP_BOTH_MAX_TRIES: u32 = 100;

/// Expand a keep-both suffix template (`{date}` → `date`) and check that
/// it yields a single, valid path-name fragment.
pub fn keep_both_suffix(template: &str, date: &str) -> Result<String> {
    let suffix = template.replace("{date}", date);
    let bad = |reason: &str| {
        Err(CoreError::Selection(format!(
            "keep-both suffix `{template}`: {reason}"
        )))
    };
    if suffix.is_empty() {
        return bad("empty");
    }
    if suffix.contains('/') || suffix.contains('\\') || suffix.contains('\0') {
        return bad("must not contain path separators or NUL");
    }
    Ok(suffix)
}

/// Compute the plan for importing `manifest` on this machine. Writes nothing.
///
/// Rules (see module docs): item keys via
/// [`ItemKey::of`](crate::key::ItemKey::of); `options.items =
/// None` selects every item not in [`Plan::sensitive`] plus every
/// sensitive item listed in `confirmed_sensitive`. Unknown item keys in `items`,
/// `confirmed_sensitive`, `item_rules` or `remaps` are a
/// [`CoreError::Selection`]; per-file rules that match no target only warn.
///
/// Action semantics apply must honour:
/// - `Replace` on a [`ObjectKind::Dir`] only means "set the mode": a
///   directory is never replaced (dir ↔ non-dir conflicts are `Skip`).
/// - `KeepBoth { rel }` is always a sibling of `rel` (same parent, same
///   root); `abs` stays the conflicting original target.
pub fn plan(
    platform: &dyn Platform,
    recipes: &[Recipe],
    manifest: &Manifest,
    options: &PlanOptions,
    session: &SessionId,
) -> Result<Plan> {
    plan_on_date(
        platform,
        recipes,
        manifest,
        options,
        session,
        &time::date(time::now_unix()),
    )
}

/// [`plan`] with an explicit `{date}` for keep-both names (deterministic).
pub fn plan_on_date(
    platform: &dyn Platform,
    recipes: &[Recipe],
    manifest: &Manifest,
    options: &PlanOptions,
    session: &SessionId,
    date: &str,
) -> Result<Plan> {
    plan_with_plugins(platform, recipes, &[], manifest, options, session, date)
}

/// [`plan_on_date`] with the `plugins` available here: an item whose local
/// recipe item names a plugin has its files passed through it
/// ([`Plan::plugins`]) once the user allowed it
/// ([`PlanOptions::confirmed_plugins`], else [`Blocker::UnconfirmedPlugin`]);
/// with the plugin missing, its entries are skipped with a warning.
pub fn plan_with_plugins(
    platform: &dyn Platform,
    recipes: &[Recipe],
    plugins: &[Plugin],
    manifest: &Manifest,
    options: &PlanOptions,
    session: &SessionId,
    date: &str,
) -> Result<Plan> {
    planner::Planner::new(platform, recipes, plugins, manifest, options, date)?.run(session)
}
