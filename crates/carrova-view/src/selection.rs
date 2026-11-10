//! Presentation model of the export selection: which items are shown, what
//! the scan knows about each and which marks they carry. Pure (no I/O, no
//! escaping).

use serde::Serialize;
use std::collections::HashMap;

use carrova_core::select::{ItemChoice, Selection, SelectionIssue};
use carrova_core::{ItemKey, RootedPath, ScanReport};

/// Label shown for an unrecognised (raw) directory.
pub const RAW_LABEL: &str = "unrecognised data";

/// What the scan found for one selectable item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ItemFacts<'a> {
    /// Recipe item label, or [`RAW_LABEL`] for raw dirs.
    pub label: &'a str,
    /// Bytes in regular files.
    pub size: u64,
    /// Regular files.
    pub files: u64,
}

/// Item key → [`ItemFacts`] for every detected recipe item and raw dir of
/// `scan`.
pub fn facts_by_key(scan: &ScanReport) -> HashMap<ItemKey, ItemFacts<'_>> {
    let recipe_items = scan.apps.iter().filter(|a| a.detected).flat_map(|app| {
        app.items.iter().map(move |i| {
            let facts = ItemFacts {
                label: &i.label,
                size: i.size,
                files: i.file_count,
            };
            (ItemKey::recipe(&app.id, &i.id), facts)
        })
    });
    let raw_dirs = scan.unrecognised.iter().map(|raw| {
        let facts = ItemFacts {
            label: RAW_LABEL,
            size: raw.size,
            files: raw.file_count,
        };
        (ItemKey::raw(RootedPath::new(raw.root, &raw.rel)), facts)
    });
    recipe_items.chain(raw_dirs).collect()
}

/// A mark next to a selection row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SelectionTag<'a> {
    /// Carrova suggests ticking it.
    Suggested,
    /// Sensitive; `confirmed` once the user confirmed it.
    Sensitive { confirmed: bool },
    /// The recipe marks it unmovable; `forced` when the user overrode that.
    Unmovable { reason: &'a str, forced: bool },
}

/// One shown item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SelectionRow<'a> {
    /// Item key.
    pub key: &'a ItemKey,
    /// Ticked.
    pub included: bool,
    /// What the scan found, if it knows the item.
    pub facts: Option<ItemFacts<'a>>,
    /// Marks, in display order.
    pub tags: Vec<SelectionTag<'a>>,
    /// Paths the user unticked inside the item.
    pub excluded_paths: &'a [String],
    /// What the engine's selection rules make of the item.
    pub roles: ItemRoles,
}

/// How the engine's selection rules treat an item right now (its
/// [`ItemChoice`] predicates), so front-ends group rows exactly as the
/// edits ([`carrova_core::SelectionEdit`]) act on them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ItemRoles {
    /// Neither sensitive nor unmovable: an app's row and presets tick it
    /// ([`ItemChoice::is_ordinary`]).
    pub ordinary: bool,
    /// May be ticked without an override ([`ItemChoice::is_movable`]).
    pub movable: bool,
    /// Lives under the cache root ([`ItemChoice::is_cache`]).
    pub cache: bool,
}

impl ItemRoles {
    /// The roles of `choice`.
    #[must_use]
    pub fn of(choice: &ItemChoice) -> Self {
        Self {
            ordinary: choice.is_ordinary(),
            movable: choice.is_movable(),
            cache: choice.is_cache(),
        }
    }
}

/// The selection as shown before export or send.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SelectionView<'a> {
    /// Every recipe item and every ticked raw dir, in key order.
    pub rows: Vec<SelectionRow<'a>>,
    /// Raw dirs not ticked (not listed, only counted).
    pub hidden_raw: usize,
    /// Items ticked.
    pub included: usize,
    /// Scanned bytes of the ticked items (before path exclusions).
    pub included_size: u64,
    /// Why the selection cannot be used as is.
    pub issues: Vec<SelectionIssue>,
}

/// View of `sel`, with sizes from `scan`.
pub fn selection_view<'a>(sel: &'a Selection, scan: &'a ScanReport) -> SelectionView<'a> {
    let facts = facts_by_key(scan);
    let rows: Vec<SelectionRow<'a>> = sel
        .items
        .iter()
        .filter(|(k, c)| c.included || !k.is_raw())
        .map(|(key, c)| SelectionRow {
            key,
            included: c.included,
            facts: facts.get(key).copied(),
            tags: tags(c),
            excluded_paths: &c.excluded_paths,
            roles: ItemRoles::of(c),
        })
        .collect();
    SelectionView {
        hidden_raw: sel
            .items
            .iter()
            .filter(|(k, c)| k.is_raw() && !c.included)
            .count(),
        included: sel.included().count(),
        included_size: rows
            .iter()
            .filter(|r| r.included)
            .filter_map(|r| r.facts.map(|f| f.size))
            .sum(),
        issues: sel.issues(),
        rows,
    }
}

fn tags(c: &ItemChoice) -> Vec<SelectionTag<'_>> {
    let suggested = c.suggested.then_some(SelectionTag::Suggested);
    let sensitive = c.sensitive.then_some(SelectionTag::Sensitive {
        confirmed: c.confirmed_sensitive,
    });
    let unmovable = c
        .unmovable
        .as_deref()
        .map(|reason| SelectionTag::Unmovable {
            reason,
            forced: c.force_unmovable,
        });
    [suggested, sensitive, unmovable]
        .into_iter()
        .flatten()
        .collect()
}

#[cfg(test)]
#[path = "../tests/selection/unit.rs"]
mod tests;
