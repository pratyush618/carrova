//! Item checks, including sensitive-path capture across items.

use std::collections::HashMap;

use super::{IssueKind, Sink, is_kebab};
use crate::{Item, Recipe, build_excludes};

/// At least one item; unique item ids; each item on its own.
pub(super) fn check_items(r: &Recipe, s: &mut Sink<'_>) {
    if r.items.is_empty() {
        s.push("items", IssueKind::NoItems);
    }
    let mut item_ids: HashMap<&str, usize> = HashMap::new();
    for (i, item) in r.items.iter().enumerate() {
        let duplicate = item_ids.insert(&item.id, i).is_some();
        check_item(r.schema, i, item, duplicate, s);
    }
}

/// One item (`items[i]`); `duplicate` if an earlier item has its id.
fn check_item(schema: u32, i: usize, item: &Item, duplicate: bool, s: &mut Sink<'_>) {
    let at = |field: &str| format!("items[{i}].{field}");
    if !is_kebab(&item.id) {
        s.push(
            at("id"),
            IssueKind::NotKebab {
                value: item.id.clone(),
            },
        );
    }
    if duplicate {
        s.push(
            at("id"),
            IssueKind::DuplicateItemId {
                id: item.id.clone(),
            },
        );
    }
    if item.label.trim().is_empty() {
        s.push(at("label"), IssueKind::Empty);
    }
    if item.unmovable.is_none() && item.paths.is_empty() {
        s.push(at("paths"), IssueKind::NoPaths);
    }
    if item
        .unmovable
        .as_deref()
        .is_some_and(|u| u.trim().is_empty())
    {
        s.push(at("unmovable"), IssueKind::Empty);
    }
    if item.handler.is_some() {
        if schema < 2 {
            s.push(
                at("handler"),
                IssueKind::RequiresSchema {
                    field: "handler",
                    min: 2,
                },
            );
        }
        if item.unmovable.is_some() {
            s.push(at("handler"), IssueKind::UnmovableWithHandler);
        }
    }
    check_excludes(i, item, s);
    if item.flags.default == Some(true) {
        if item.flags.sensitive {
            s.push(at("flags.default"), IssueKind::SensitiveDefaultOn);
        }
        if item.unmovable.is_some() {
            s.push(at("flags.default"), IssueKind::UnmovableDefaultOn);
        }
    }
}

/// Exclude globs of `items[i]`: non-empty and valid.
fn check_excludes(i: usize, item: &Item, s: &mut Sink<'_>) {
    for (g, glob) in item.exclude.iter().enumerate() {
        if glob.is_empty() {
            s.push(format!("items[{i}].exclude[{g}]"), IssueKind::Empty);
        } else if let Err(e) = build_excludes(std::slice::from_ref(glob)) {
            s.push(
                format!("items[{i}].exclude[{g}]"),
                IssueKind::InvalidGlob {
                    glob: glob.clone(),
                    message: e.to_string(),
                },
            );
        }
    }
}

/// Sensitive data must never be captured by a non-sensitive item.
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
    for (qi, q_item) in r.items.iter().enumerate() {
        if q_item.flags.sensitive || !q_item.is_movable() {
            continue;
        }
        // Invalid globs are reported above; resolve captures nothing then.
        let Ok(excludes) = build_excludes(&q_item.exclude) else {
            continue;
        };
        for (qj, q) in q_item.paths.iter().enumerate() {
            let q_comps: Vec<&str> = q.rel.components().collect();
            for p_item in r.items.iter().filter(|it| it.flags.sensitive) {
                for p in p_item.paths.iter().filter(|p| p.root == q.root) {
                    let p_comps: Vec<&str> = p.rel.components().collect();
                    if !p_comps.starts_with(&q_comps) {
                        continue;
                    }
                    // Walk order: Q itself, then each descendant down to P.
                    let excluded = (q_comps.len()..=p_comps.len()).any(|n| {
                        let rel_item = p_comps[q_comps.len()..n].join("/");
                        let rel_root = p_comps[..n].join("/");
                        (!rel_item.is_empty() && excludes.is_match(&rel_item))
                            || excludes.is_match(&rel_root)
                    });
                    if !excluded {
                        s.push(
                            format!("items[{qi}].paths[{qj}]"),
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
    }
}
