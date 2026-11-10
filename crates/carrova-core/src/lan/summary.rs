//! The receiver's plan as shown on the sender ([`PlanSummary`]): item keys,
//! action counts and conflicting targets only, never the receiver's file
//! listing.

use carrova_platform::SemanticRoot;
use serde::{Deserialize, Serialize};

use crate::key::ItemKey;
use crate::plan::{Action, Plan};

/// Receiver plan as shown on the sender: selected items only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PlanSummary {
    /// One entry per selected item, in plan order.
    pub items: Vec<ItemPlanSummary>,
}

/// Planned actions for one item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ItemPlanSummary {
    /// Item key (`recipe/item` or raw key).
    pub item: ItemKey,
    /// Number of paths per planned action.
    pub counts: ActionCounts,
    /// The item's conflicts as defined by [`Plan::conflicts`] (targets
    /// replaced, kept both or skipped). No other receiver path is listed.
    pub conflicts: Vec<ConflictTarget>,
}

impl PlanSummary {
    /// Summary of `plan` for the sender: one entry per planned item (plan
    /// order; handler-only items included), action counts and
    /// [`Plan::conflicts`]. Nothing else about the receiver's files.
    #[must_use]
    pub fn from_plan(plan: &Plan) -> Self {
        let mut items: Vec<ItemPlanSummary> = Vec::new();
        for a in &plan.actions {
            let c = &mut entry(&mut items, &a.item).counts;
            let n = match a.action {
                Action::Create => &mut c.create,
                Action::Replace => &mut c.replace,
                Action::KeepBoth { .. } => &mut c.keep_both,
                Action::Skip { .. } => &mut c.skip,
                Action::Identical => &mut c.identical,
            };
            *n += 1;
        }
        for h in &plan.handlers {
            entry(&mut items, &h.item);
        }
        for a in plan.conflicts() {
            entry(&mut items, &a.item).conflicts.push(ConflictTarget {
                root: a.root,
                rel: a.rel.clone(),
            });
        }
        Self { items }
    }
}

/// Paths per planned action kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ActionCounts {
    /// Target absent: created.
    pub create: u64,
    /// Target replaced (old one backed up).
    pub replace: u64,
    /// Written next to the existing target.
    pub keep_both: u64,
    /// Not written.
    pub skip: u64,
    /// Already identical.
    pub identical: u64,
}

/// A conflicting target on the receiver, as a semantic root + relative path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ConflictTarget {
    /// Semantic root of the target.
    pub root: SemanticRoot,
    /// Path relative to `root`.
    pub rel: String,
}

/// The summary entry of `item`, appended if new.
fn entry<'a>(items: &'a mut Vec<ItemPlanSummary>, item: &ItemKey) -> &'a mut ItemPlanSummary {
    let i = match items.iter().position(|s| s.item == *item) {
        Some(i) => i,
        None => {
            items.push(ItemPlanSummary {
                item: item.clone(),
                counts: ActionCounts::default(),
                conflicts: Vec::new(),
            });
            items.len() - 1
        }
    };
    &mut items[i]
}
