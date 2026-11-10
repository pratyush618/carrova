//! Sensitive-path capture across items, checked on every OS.

use carrova_platform::OsKind;
use globset::GlobSet;

use super::{IssueKind, Sink};
use crate::{Item, Recipe, build_excludes};

/// Sensitive data must never be captured by a non-sensitive item, on any
/// OS (each OS uses its own [`Item::paths_for`] / [`Item::exclude_for`]).
///
/// For every sensitive item path `P` and every path `Q` of a movable,
/// non-sensitive item with `P` equal to or under `Q`, `Q`'s excludes must
/// cover `P` — with the matching rules [`crate::resolve_item`] uses: a glob
/// matches the path relative to `Q` or relative to the root, and an excluded
/// directory prunes its subtree (so excluding any ancestor of `P` at or
/// below `Q` also counts).
///
/// Limitation: only paths under the *same* semantic root are compared. Roots
/// may nest on a given machine (e.g. `user_config` is usually `home/.config`),
/// but that is machine-dependent, so cross-root overlap is not detected here.
pub(super) fn check_sensitive_capture(r: &Recipe, s: &mut Sink<'_>) {
    for os in OsKind::ALL {
        for (qi, q_item) in r.items.iter().enumerate() {
            if !q_item.flags.sensitive && q_item.is_movable() {
                check_item_on(os, r, qi, q_item, s);
            }
        }
    }
}

/// Whether non-sensitive `q_item` (`items[qi]`) captures a sensitive path
/// on `os`.
fn check_item_on(os: OsKind, r: &Recipe, qi: usize, q_item: &Item, s: &mut Sink<'_>) {
    // Invalid globs are reported elsewhere; resolve captures nothing then.
    let Ok(excludes) = build_excludes(q_item.exclude_for(os)) else {
        return;
    };
    let overridden = q_item.os.as_ref().is_some_and(|o| o.get(os).is_some());
    for (qj, q) in q_item.paths_for(os).iter().enumerate() {
        let q_comps: Vec<&str> = q.rel.components().collect();
        for p_item in r.items.iter().filter(|it| it.flags.sensitive) {
            for p in p_item.paths_for(os).iter().filter(|p| p.root == q.root) {
                let p_comps: Vec<&str> = p.rel.components().collect();
                if !p_comps.starts_with(&q_comps) || is_excluded(&excludes, &q_comps, &p_comps) {
                    continue;
                }
                let location = if overridden {
                    format!("items[{qi}].os.{}.paths[{qj}]", os.as_str())
                } else {
                    format!("items[{qi}].paths[{qj}]")
                };
                s.push(
                    location,
                    IssueKind::SensitiveCapturedByNonSensitive {
                        path: p.clone(),
                        sensitive_item: p_item.id.clone(),
                        by_item: q_item.id.clone(),
                    },
                );
            }
        }
    }
}

/// Whether `excludes` prune sensitive path `p` (components) somewhere on
/// the walk down from item path `q`: `Q` itself, then each descendant.
fn is_excluded(excludes: &GlobSet, q: &[&str], p: &[&str]) -> bool {
    (q.len()..=p.len()).any(|n| {
        let rel_item = p[q.len()..n].join("/");
        let rel_root = p[..n].join("/");
        (!rel_item.is_empty() && excludes.is_match(&rel_item)) || excludes.is_match(&rel_root)
    })
}
