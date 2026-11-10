//! Item checks: identity, paths, handler, excludes, flags.

use std::collections::HashMap;

use super::os_paths::check_os_overrides;
use super::{IssueKind, Sink, is_kebab, since};
use crate::{HandlerRef, Item, Recipe, build_excludes};

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
    if let Some(handler) = &item.handler {
        check_handler(schema, i, item, handler, s);
    }
    check_excludes(&format!("items[{i}].exclude"), &item.exclude, s);
    if let Some(overrides) = &item.os {
        check_os_overrides(schema, i, item, overrides, s);
    }
    if item.flags.default == Some(true) {
        if item.flags.sensitive {
            s.push(at("flags.default"), IssueKind::SensitiveDefaultOn);
        }
        if item.unmovable.is_some() {
            s.push(at("flags.default"), IssueKind::UnmovableDefaultOn);
        }
    }
}

/// `items[i].handler`: built-ins need schema 2, plugins schema 3; never
/// on an unmovable item.
fn check_handler(schema: u32, i: usize, item: &Item, handler: &HandlerRef, s: &mut Sink<'_>) {
    let at = format!("items[{i}].handler");
    s.require_schema(schema, at.clone(), "handler", since::HANDLER);
    if matches!(handler, HandlerRef::Plugin(_)) {
        s.require_schema(schema, at.clone(), "handler.plugin", since::PLUGIN_HANDLER);
    }
    if item.unmovable.is_some() {
        s.push(at, IssueKind::UnmovableWithHandler);
    }
}

/// Exclude globs listed at `at` (e.g. `items[0].exclude`): non-empty and
/// valid.
pub(super) fn check_excludes(at: &str, globs: &[String], s: &mut Sink<'_>) {
    for (g, glob) in globs.iter().enumerate() {
        if glob.is_empty() {
            s.push(format!("{at}[{g}]"), IssueKind::Empty);
        } else if let Err(e) = build_excludes(std::slice::from_ref(glob)) {
            s.push(
                format!("{at}[{g}]"),
                IssueKind::InvalidGlob {
                    glob: glob.clone(),
                    message: e.to_string(),
                },
            );
        }
    }
}
