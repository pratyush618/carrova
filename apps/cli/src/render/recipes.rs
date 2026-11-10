//! `carrova recipes list | status | update | rollback` as text.

use std::io::{self, Write};

use carrova_bundle::{
    BundleVersion, EffectiveTier, InstallOutcome, ResolvedRecipes, SkippedRecipe, TierCounts,
    UpdateOutcome,
};
use carrova_core::time::utc_date;

use super::text::{files, human_size, safe};
use crate::recipes::status::{SourceView, StatusView};

/// Tier name as shown to the user.
#[must_use]
pub fn tier_label(tier: EffectiveTier) -> &'static str {
    match tier {
        EffectiveTier::Verified => "verified",
        EffectiveTier::Community => "community",
        EffectiveTier::Unverified => "unverified",
    }
}

/// One line per recipe (`id  name  category  tier`, the effective tier:
/// `unverified` for an unsigned tree), then the count.
pub fn recipe_list(out: &mut impl Write, resolved: &ResolvedRecipes) -> io::Result<()> {
    let recipes = &resolved.recipes;
    let id_w = recipes.iter().map(|r| r.recipe.id.len()).max().unwrap_or(0);
    let name_w = recipes
        .iter()
        .map(|r| r.recipe.name.len())
        .max()
        .unwrap_or(0);
    let cat_w = recipes
        .iter()
        .map(|r| r.recipe.category.len())
        .max()
        .unwrap_or(0);
    for bundled in recipes {
        let r = &bundled.recipe;
        writeln!(
            out,
            "{:<id_w$}  {:<name_w$}  {:<cat_w$}  {}",
            safe(&r.id),
            safe(&r.name),
            safe(&r.category),
            tier_label(resolved.effective_tier(bundled))
        )?;
    }
    writeln!(out, "{} recipes", recipes.len())
}

/// The status: source, creation date, counts by tier, skipped recipes.
pub fn status(out: &mut impl Write, s: &StatusView) -> io::Result<()> {
    let trust = if s.verified { "verified" } else { "unverified" };
    let source = match &s.source {
        SourceView::DevDir { path } => format!("developer recipe tree {}", path.display()),
        SourceView::Installed { version } => format!("installed bundle v{version}"),
        SourceView::Shipped { version, path } => {
            format!("shipped bundle v{version} ({})", path.display())
        }
    };
    writeln!(out, "Source   {} [{trust}]", safe(&source))?;
    if let Some(created) = s.created {
        writeln!(out, "Created  {} (UTC)", utc_date(created))?;
    }
    writeln!(out, "Recipes  {}", tier_breakdown(s.recipes))?;
    if !s.skipped.is_empty() {
        writeln!(out, "Skipped  {}", s.skipped.len())?;
        for k in &s.skipped {
            writeln!(out, "  - {}: {}", safe(&k.id), safe(&k.reason))?;
        }
    }
    Ok(())
}

/// `3 (2 verified, 1 community)`: the total, then each non-empty tier.
fn tier_breakdown(c: TierCounts) -> String {
    let parts: Vec<String> = [
        (c.verified, EffectiveTier::Verified),
        (c.community, EffectiveTier::Community),
        (c.unverified, EffectiveTier::Unverified),
    ]
    .into_iter()
    .filter(|&(n, _)| n > 0)
    .map(|(n, tier)| format!("{n} {}", tier_label(tier)))
    .collect();
    if parts.is_empty() {
        c.total.to_string()
    } else {
        format!("{} ({})", c.total, parts.join(", "))
    }
}

/// What `carrova recipes update` did.
pub fn update_outcome(out: &mut impl Write, outcome: &UpdateOutcome) -> io::Result<()> {
    match outcome {
        UpdateOutcome::UpToDate(v)
        | UpdateOutcome::Installed {
            outcome: InstallOutcome::AlreadyCurrent(v),
            ..
        } => writeln!(out, "Recipes are already up to date ({v})."),
        UpdateOutcome::Refused(v) => writeln!(
            out,
            "Recipe bundle {v} was rolled back earlier and is not installed again; \
             keeping the current recipes."
        ),
        UpdateOutcome::Installed {
            outcome: InstallOutcome::Installed { version, previous },
            stats,
            skipped,
        } => {
            let was = previous.map_or_else(|| "first install".to_owned(), |p| format!("was {p}"));
            writeln!(
                out,
                "Installed recipe bundle {version} ({was}): downloaded {} ({}), reused {}.",
                files(stats.fetched as u64),
                human_size(stats.bytes),
                stats.reused
            )?;
            skipped_lines(out, skipped)
        }
    }
}

/// What `carrova recipes reset` did (`removed`: the bundle that was
/// installed).
pub fn reset_outcome(out: &mut impl Write, removed: Option<BundleVersion>) -> io::Result<()> {
    match removed {
        Some(v) => writeln!(
            out,
            "Removed recipe bundle {v}; Carrova now uses the recipes it shipped with."
        ),
        None => writeln!(out, "No recipe bundle was installed; nothing to reset."),
    }
}

/// Recipes an update left out.
fn skipped_lines(out: &mut impl Write, skipped: &[SkippedRecipe]) -> io::Result<()> {
    for s in skipped {
        writeln!(
            out,
            "  skipped {}: {}",
            safe(&s.id),
            safe(&s.reason.to_string())
        )?;
    }
    Ok(())
}
