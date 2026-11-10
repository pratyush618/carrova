//! Presentation model of an import [`Plan`]: per-item action counts and the
//! lines worth showing under each item. Pure (no I/O, no escaping); the
//! decisions themselves all come from the engine's plan, this only chooses
//! how much of it to show.

use carrova_core::lan::ActionCounts;
use carrova_core::plan::{Action, ObjectKind, Plan, PlannedAction, SpaceNeed};
use carrova_platform::SemanticRoot;
use serde::Serialize;

use super::Detail;

/// Directories with fewer creates than this are listed path by path even
/// when collapsing.
pub const COLLAPSE_MIN: usize = 3;

/// Action counts and bytes to write for one archive item.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Counts {
    /// Paths per action kind (the same type the receiver sends a LAN
    /// sender in its plan summary).
    pub actions: ActionCounts,
    /// Bytes written by creates, replaces and keep-both copies.
    pub bytes: u64,
}

/// Creates under one top-level directory below an item's base, shown as a
/// single line.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct DirGroup {
    pub root: SemanticRoot,
    /// Directory path (no trailing slash).
    pub rel: String,
    /// Regular files created in it (directories and symlinks not counted).
    pub files: usize,
    pub bytes: u64,
}

/// One line under an item's summary. Tagged `line` (an action already
/// has a `kind`: file, dir or symlink).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "line", rename_all = "snake_case")]
pub enum Line<'a> {
    /// One planned action, listed as-is.
    Action(&'a PlannedAction),
    /// Collapsed creates.
    Group(DirGroup),
}

/// One archive item with its counts and lines.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ItemView<'a> {
    pub key: &'a str,
    pub counts: Counts,
    pub lines: Vec<Line<'a>>,
}

/// The whole plan, items in plan order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlanView<'a> {
    pub items: Vec<ItemView<'a>>,
    /// Some creates were collapsed into [`Line::Group`]s.
    pub collapsed: bool,
}

/// Build the view of `plan` at the given detail.
///
/// [`Detail::Full`] lists every non-identical action individually.
/// [`Detail::Summary`] lists every replace, keep-both, skip and symlink
/// individually, plus creates directly in an item's base directory; other
/// creates are collapsed per top-level directory below the base (the common
/// directory of the item's paths in that root), unless that directory holds
/// fewer than [`COLLAPSE_MIN`] creates.
pub fn plan_view(plan: &Plan, detail: Detail) -> PlanView<'_> {
    let mut collapsed = false;
    let items = first_seen(plan.actions.iter().map(|a| a.item.as_str()))
        .into_iter()
        .map(|key| {
            let actions: Vec<&PlannedAction> =
                plan.actions.iter().filter(|a| a.item == key).collect();
            let lines = match detail {
                Detail::Full => actions
                    .iter()
                    .filter(|a| a.action != Action::Identical)
                    .map(|a| Line::Action(a))
                    .collect(),
                Detail::Summary => summary_lines(&actions),
            };
            collapsed |= lines.iter().any(|l| matches!(l, Line::Group(_)));
            ItemView {
                key,
                counts: counts(&actions),
                lines,
            }
        })
        .collect();
    PlanView { items, collapsed }
}

/// Free-space check of one filesystem the plan writes to.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FsSpace {
    /// What lives on it (`"the state dir and home"`).
    pub what: String,
    /// Free space it needs (payload plus margin).
    pub needed: u64,
    /// Free space it has, if known.
    pub available: Option<u64>,
    /// How much is missing, when known to be short.
    pub short_by: Option<u64>,
}

/// Space the plan writes and needs, per filesystem (state-dir filesystem
/// first, as the engine orders them).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SpaceView {
    /// Payload bytes the plan writes.
    pub writes: u64,
    /// One entry per filesystem written to.
    pub filesystems: Vec<FsSpace>,
}

/// View of the plan's space needs as computed by the engine
/// (`carrova_core::plan::space_needs`).
pub fn space_view(plan: &Plan, needs: &[SpaceNeed]) -> SpaceView {
    SpaceView {
        writes: plan.bytes_to_write,
        filesystems: needs
            .iter()
            .map(|n| FsSpace {
                what: n.describe(),
                needed: n.needed,
                available: n.available,
                short_by: n
                    .is_short()
                    .then(|| n.needed.saturating_sub(n.available.unwrap_or(0))),
            })
            .collect(),
    }
}

fn counts(actions: &[&PlannedAction]) -> Counts {
    let mut c = Counts::default();
    for a in actions {
        let n = &mut c.actions;
        match a.action {
            Action::Create => n.create += 1,
            Action::Replace => n.replace += 1,
            Action::KeepBoth { .. } => n.keep_both += 1,
            Action::Skip { .. } => n.skip += 1,
            Action::Identical => n.identical += 1,
        }
        if writes(a) {
            c.bytes += a.size;
        }
    }
    c
}

fn writes(a: &PlannedAction) -> bool {
    matches!(
        a.action,
        Action::Create | Action::Replace | Action::KeepBoth { .. }
    )
}

fn components(rel: &str) -> Vec<&str> {
    rel.split('/').filter(|c| !c.is_empty()).collect()
}

/// Common directory of `actions` (all in one root): the shared component
/// prefix, minus its last component if that is itself a file or symlink.
fn base<'a>(actions: &[&'a PlannedAction]) -> Vec<&'a str> {
    let mut paths = actions.iter().map(|a| components(&a.rel));
    let Some(mut base) = paths.next() else {
        return Vec::new();
    };
    for p in paths {
        let shared = base.iter().zip(&p).take_while(|(x, y)| x == y).count();
        base.truncate(shared);
    }
    let base_is_leaf = actions
        .iter()
        .any(|a| a.kind != ObjectKind::Dir && components(&a.rel).len() == base.len());
    if base_is_leaf {
        base.pop();
    }
    base
}

/// Distinct values of `values`, in first-seen order.
fn first_seen<T: PartialEq>(values: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut out = Vec::new();
    for v in values {
        if !out.contains(&v) {
            out.push(v);
        }
    }
    out
}

/// Creates under one directory, before deciding whether to collapse them.
struct PendingGroup<'a> {
    root: SemanticRoot,
    rel: String,
    members: Vec<&'a PlannedAction>,
}

impl<'a> PendingGroup<'a> {
    /// One [`Line::Group`], or each member as-is when there are fewer than
    /// [`COLLAPSE_MIN`].
    fn into_lines(self) -> Vec<Line<'a>> {
        if self.members.len() < COLLAPSE_MIN {
            return self.members.into_iter().map(Line::Action).collect();
        }
        let files: Vec<_> = self
            .members
            .iter()
            .filter(|a| a.kind == ObjectKind::File)
            .collect();
        vec![Line::Group(DirGroup {
            root: self.root,
            rel: self.rel,
            files: files.len(),
            bytes: files.iter().map(|a| a.size).sum(),
        })]
    }
}

/// Component count of each root's [`base`] directory.
fn base_lens(actions: &[&PlannedAction]) -> Vec<(SemanticRoot, usize)> {
    first_seen(actions.iter().map(|a| a.root))
        .into_iter()
        .map(|r| {
            let in_root: Vec<&PlannedAction> =
                actions.iter().copied().filter(|a| a.root == r).collect();
            (r, base(&in_root).len())
        })
        .collect()
}

fn summary_lines<'a>(actions: &[&'a PlannedAction]) -> Vec<Line<'a>> {
    let bases = base_lens(actions);
    let mut singles: Vec<Line<'a>> = Vec::new();
    let mut groups: Vec<PendingGroup<'a>> = Vec::new();
    let mut attention: Vec<Line<'a>> = Vec::new();
    for a in actions.iter().copied() {
        match a.action {
            Action::Identical => {}
            Action::Create if a.kind != ObjectKind::Symlink => {
                let base_len = bases
                    .iter()
                    .find(|(r, _)| *r == a.root)
                    .map_or(0, |(_, len)| *len);
                let parts = components(&a.rel);
                let below = parts.len().saturating_sub(base_len);
                if below == 0 || (below == 1 && a.kind != ObjectKind::Dir) {
                    singles.push(Line::Action(a));
                    continue;
                }
                let rel = parts[..=base_len].join("/");
                match groups.iter_mut().find(|g| g.root == a.root && g.rel == rel) {
                    Some(g) => g.members.push(a),
                    None => groups.push(PendingGroup {
                        root: a.root,
                        rel,
                        members: vec![a],
                    }),
                }
            }
            _ => attention.push(Line::Action(a)),
        }
    }
    singles
        .into_iter()
        .chain(groups.into_iter().flat_map(PendingGroup::into_lines))
        .chain(attention)
        .collect()
}

#[cfg(test)]
#[path = "../tests/plan/unit.rs"]
mod tests;
