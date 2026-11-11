//! End-of-import checklist from the applied plan.

use std::collections::BTreeMap;

use carrova_manifest::Manifest;
use carrova_recipe::Recipe;

use crate::checklist::{Checklist, Skipped};
use crate::key::ItemKey;
use crate::plan::{Action, Plan};

/// The checklist of an import that applied `plan` (from `manifest`).
pub(super) fn import_checklist(
    manifest: &Manifest,
    plan: &Plan,
    recipes: &[Recipe],
    rollback_available: bool,
) -> Checklist {
    let mut skipped = skipped_actions(plan);
    skipped.extend(unselected_items(manifest, plan));
    Checklist {
        moved: moved_items(plan),
        skipped,
        apps_to_install: apps_to_install(plan, recipes),
        rollback_available,
    }
}

/// Items with at least one written (or already identical) path or a
/// handler, in plan order, each once.
fn moved_items(plan: &Plan) -> Vec<ItemKey> {
    let written = plan
        .actions
        .iter()
        .filter(|a| !matches!(a.action, Action::Skip { .. }))
        .map(|a| &a.item);
    let mut moved: Vec<ItemKey> = Vec::new();
    for item in written.chain(plan.handlers.iter().map(|h| &h.item)) {
        if !moved.contains(item) {
            moved.push(item.clone());
        }
    }
    moved
}

/// Skipped paths per `(item, reason)`, in first-seen order, with a path
/// count when several paths share one.
fn skipped_actions(plan: &Plan) -> Vec<Skipped> {
    let mut counts: Vec<((&str, &str), usize)> = Vec::new();
    for a in &plan.actions {
        if let Action::Skip { reason } = &a.action {
            let key = (a.item.as_str(), reason.as_str());
            match counts.iter_mut().find(|(k, _)| *k == key) {
                Some((_, n)) => *n += 1,
                None => counts.push((key, 1)),
            }
        }
    }
    counts
        .into_iter()
        .map(|((what, reason), n)| Skipped {
            what: what.to_owned(),
            reason: if n == 1 {
                reason.to_owned()
            } else {
                format!("{reason} ({n} paths)")
            },
        })
        .collect()
}

/// Manifest items the plan left out, and why.
fn unselected_items<'a>(
    manifest: &'a Manifest,
    plan: &'a Plan,
) -> impl Iterator<Item = Skipped> + 'a {
    manifest.items.iter().filter_map(move |item| {
        let key = ItemKey::of(item);
        let selected = plan.picks(item);
        let unconfirmed = plan.sensitivity(item).is_some() && !plan.confirms(&key);
        (!selected).then(|| Skipped {
            reason: if unconfirmed {
                "sensitive item not confirmed".into()
            } else {
                "not selected".into()
            },
            what: key.to_string(),
        })
    })
}

/// Names (recipe name, else id) of the apps the plan found missing.
fn apps_to_install(plan: &Plan, recipes: &[Recipe]) -> Vec<String> {
    let names: BTreeMap<&str, &str> = recipes
        .iter()
        .map(|r| (r.id.as_str(), r.name.as_str()))
        .collect();
    plan.missing_apps
        .iter()
        .map(|id| {
            names
                .get(id.as_str())
                .map_or_else(|| id.clone(), |n| (*n).to_owned())
        })
        .collect()
}
