//! Transfer priority of the chunks a receiver fetches.

use std::collections::{BTreeMap, HashSet};

use carrova_manifest::{ChunkRef, Manifest};
use carrova_platform::SemanticRoot;

use crate::error::Result;
use crate::key::ItemKey;
use crate::plan::Plan;
use crate::stage::StageLayout;

/// Chunks the receiver must fetch for `plan`, deduplicated, in transfer
/// priority order: small things first, so most items are complete early.
///
/// Items are ordered by `(late, total staged bytes, plan order)`, where
/// `late` items are raw (recipe-less, unrecognised) items and items whose every file
/// lands in the `cache` root; within an item, files in plan order (handler
/// artifacts last), chunks by offset. A chunk shared with an earlier item
/// is fetched with that item. Only what staging needs is listed (the same
/// stage layout as staging). Filter the result with the staging store's
/// `missing` (order is preserved) to skip chunks already staged. A plan
/// that does not match `manifest` is a [`crate::CoreError::Verification`]
/// error.
pub fn transfer_order(plan: &Plan, manifest: &Manifest) -> Result<Vec<ChunkRef>> {
    let layout = StageLayout::new(plan, manifest)?;
    let raw: HashSet<ItemKey> = manifest
        .items
        .iter()
        .filter(|i| i.recipe_id.is_none())
        .map(ItemKey::of)
        .collect();
    /// One item's staged files, in layout order.
    struct Group {
        late: bool,
        bytes: u64,
        first: usize,
        files: Vec<usize>,
    }
    let mut groups: BTreeMap<&ItemKey, Group> = BTreeMap::new();
    for (n, f) in layout.files().iter().enumerate() {
        let item = f.target.item();
        let g = groups.entry(item).or_insert_with(|| Group {
            late: raw.contains(item),
            bytes: 0,
            first: n,
            files: Vec::new(),
        });
        g.bytes = g.bytes.saturating_add(f.size);
        g.files.push(n);
    }
    for g in groups.values_mut() {
        let all_cache = g
            .files
            .iter()
            .all(|&n| layout.files()[n].root == Some(SemanticRoot::Cache));
        g.late |= all_cache;
    }
    let mut ordered: Vec<Group> = groups.into_values().collect();
    ordered.sort_by_key(|g| (g.late, g.bytes, g.first));

    let mut seen = HashSet::new();
    let mut out = Vec::with_capacity(layout.unique_chunk_count());
    for g in &ordered {
        for &n in &g.files {
            for c in &layout.files()[n].chunks {
                if seen.insert(c.hash) {
                    out.push(*c);
                }
            }
        }
    }
    Ok(out)
}
