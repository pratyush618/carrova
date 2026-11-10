//! Schema v3 per-OS item overrides (`items[i].os`).

use super::items::check_excludes;
use super::{IssueKind, Sink, since};
use crate::{Item, OsOverrides};

/// `items[i].os`: schema 3+; each override slot-aligned with the base
/// `paths` (same length) and with valid excludes. Override paths are
/// [`crate::RelPath`]s, so the relative-path rules hold by construction.
pub(super) fn check_os_overrides(
    schema: u32,
    i: usize,
    item: &Item,
    overrides: &OsOverrides,
    s: &mut Sink<'_>,
) {
    s.require_schema(schema, format!("items[{i}].os"), "os", since::OS_OVERRIDES);
    for (os, o) in overrides.iter() {
        let at = format!("items[{i}].os.{}", os.as_str());
        if o.paths.len() != item.paths.len() {
            s.push(
                format!("{at}.paths"),
                IssueKind::OsPathsMisaligned {
                    os,
                    base: item.paths.len(),
                    found: o.paths.len(),
                },
            );
        }
        if let Some(exclude) = &o.exclude {
            check_excludes(&format!("{at}.exclude"), exclude, s);
        }
    }
}
