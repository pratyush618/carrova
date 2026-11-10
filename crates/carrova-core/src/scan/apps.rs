//! Recognised apps: detection, item sizes and running processes.

use std::collections::BTreeMap;
use std::path::PathBuf;

use carrova_platform::{FileKind, OsKind, Platform, ProcessInfo, SemanticRoot};
use carrova_recipe::{
    Detection, Evidence, Item, Portability, Recipe, ResolvedItem, detect, resolve_item,
};
use tracing::{debug, warn};

use super::{
    AppReport, ItemFlags, ItemPortability, ItemReport, ScanError, ScanOptions, SkippedPath, Totals,
};
use crate::cancel::Cancel;
use crate::key::RootedPath;

/// Apps found, their totals, and the absolute item paths they claim.
pub(super) struct Recognised {
    pub(super) apps: Vec<AppReport>,
    pub(super) totals: Totals,
    pub(super) claims: Vec<PathBuf>,
}

/// Detect every recipe (sorted by `(category, id)`) and report the items
/// of detected apps; `cancel` is checked before each recipe's items.
pub(super) fn recognise(
    recipes: &[Recipe],
    platform: &dyn Platform,
    roots: &BTreeMap<SemanticRoot, PathBuf>,
    opts: ScanOptions,
    cancel: &Cancel,
    warnings: &mut Vec<String>,
) -> Result<Recognised, ScanError> {
    let mut ordered: Vec<&Recipe> = recipes.iter().collect();
    ordered.sort_by(|a, b| (&a.category, &a.id).cmp(&(&b.category, &b.id)));
    let detections: Vec<Detection> = ordered.iter().map(|r| detect(r, platform)).collect();
    debug!(
        detected = detections.iter().filter(|d| d.detected).count(),
        "detection done"
    );
    let procs = processes_if_needed(&ordered, &detections, platform, warnings);

    let mut out = Recognised {
        apps: Vec::new(),
        totals: Totals::default(),
        claims: Vec::new(),
    };
    for (recipe, detection) in ordered.into_iter().zip(detections) {
        ScanError::check(cancel)?;
        if !detection.detected {
            if opts.include_undetected {
                out.apps.push(app_shell(recipe, false, Vec::new()));
            }
            continue;
        }
        let running = procs
            .as_deref()
            .is_some_and(|p| is_running(recipe, platform.os(), p));
        let mut app = app_shell(
            recipe,
            true,
            detection.evidence.iter().map(describe).collect(),
        );
        app.running = running;
        for item in &recipe.items {
            out.claims.extend(
                item.paths_for(platform.os())
                    .iter()
                    .filter_map(|pr| roots.get(&pr.root).map(|base| pr.rel.join_to(base))),
            );
            let report = item_report(recipe, item, &resolve_item(item, platform), running);
            out.totals.recognised_size += report.size;
            if report.flags.default_selected {
                out.totals.default_selected_size += report.size;
                out.totals.default_selected_files += report.file_count;
            }
            app.items.push(report);
        }
        out.apps.push(app);
    }
    Ok(out)
}

/// Running processes, listed only if some detected app has a movable item
/// that requires it closed (`None` otherwise). A failed listing is a
/// warning and counts as nothing running.
fn processes_if_needed(
    recipes: &[&Recipe],
    detections: &[Detection],
    platform: &dyn Platform,
    warnings: &mut Vec<String>,
) -> Option<Vec<ProcessInfo>> {
    let needed = recipes.iter().zip(detections).any(|(r, d)| {
        d.detected
            && r.items
                .iter()
                .any(|i| i.is_movable() && i.flags.requires_closed)
    });
    if !needed {
        return None;
    }
    Some(platform.running_processes().unwrap_or_else(|e| {
        warn!(error = %e, "cannot list running processes");
        warnings.push(format!("cannot list running processes: {e}"));
        Vec::new()
    }))
}

fn app_shell(recipe: &Recipe, detected: bool, evidence: Vec<String>) -> AppReport {
    AppReport {
        id: recipe.id.clone(),
        name: recipe.name.clone(),
        category: recipe.category.clone(),
        detected,
        evidence,
        running: false,
        items: Vec::new(),
    }
}

fn describe(e: &Evidence) -> String {
    match e {
        Evidence::Binary { name, path } => format!("binary `{name}` at {}", path.display()),
        Evidence::Path { path, kind } => {
            let kind = match kind {
                FileKind::File => "file",
                FileKind::Dir => "dir",
                FileKind::Symlink => "symlink",
                FileKind::Other => "special file",
            };
            format!("{kind} {path}")
        }
    }
}

fn item_report(
    recipe: &Recipe,
    item: &Item,
    resolved: &ResolvedItem,
    app_running: bool,
) -> ItemReport {
    let file_count = resolved
        .entries
        .iter()
        .filter(|e| e.kind == FileKind::File)
        .count();
    ItemReport {
        id: item.id.clone(),
        label: item.label.clone(),
        size: resolved.total_size,
        file_count: u64::try_from(file_count).unwrap_or(u64::MAX),
        flags: ItemFlags {
            sensitive: item.flags.sensitive,
            requires_closed: item.flags.requires_closed,
            portability: match item.flags.portability {
                Portability::SameOs => ItemPortability::SameOs,
                Portability::CrossOs => ItemPortability::CrossOs,
            },
            default_selected: item.default_selected(),
        },
        sensitive_reason: item
            .flags
            .sensitive
            .then(|| crate::plan::recipe_sensitive(&recipe.id, &item.id)),
        paths: item.paths.iter().map(ToString::to_string).collect(),
        unmovable: item.unmovable.clone(),
        missing: resolved.missing.iter().map(ToString::to_string).collect(),
        skipped: resolved
            .skipped
            .iter()
            .map(|s| SkippedPath {
                path: RootedPath::new(s.root, s.rel.as_str()).to_string(),
                reason: s.reason.to_string(),
            })
            .collect(),
        app_running: app_running && item.is_movable() && item.flags.requires_closed,
    }
}

fn is_running(recipe: &Recipe, os: OsKind, procs: &[ProcessInfo]) -> bool {
    recipe
        .process_names(os)
        .any(|n| procs.iter().any(|p| p.matches_binary(n)))
}
