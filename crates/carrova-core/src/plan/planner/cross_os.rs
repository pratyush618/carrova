//! Archives from another OS family: same-OS items are blocked (unless the
//! user overrides it per item), cross-OS items are rebased slot by slot
//! onto the local recipe item's paths for this OS.
//!
//! The source's slots ([`ManifestItem::source_paths`]) come from the
//! archive, but only choose *which* local slot an entry lands in: every
//! target is the local recipe's path plus the entry's remainder, confined
//! and validated like any other target.

use std::fmt;

use carrova_manifest::{Entry, ManifestItem, Portability};
use carrova_platform::{OsKind, SemanticRoot};

use super::Planner;
use crate::key::{ItemKey, RootedPath};
use crate::plan::{Blocker, PathMapping};

/// A cross-OS item rebased onto this OS's slots.
pub(super) struct Rebased {
    /// Per entry (manifest order): its target before any user remap.
    entries: Vec<(SemanticRoot, String)>,
    /// Every slot, source path → local path.
    pub(super) slots: Vec<PathMapping>,
}

/// Why a cross-OS item cannot be rebased: it is skipped, never guessed.
enum Unmapped {
    NoRecipe,
    Misaligned { source: usize, local: usize },
    NoSlot(String),
}

impl fmt::Display for Unmapped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("not restored: ")?;
        match self {
            Self::NoRecipe => f.write_str(
                "no recipe item here says where its data lives on this OS (remap it to choose a place)",
            ),
            Self::Misaligned { source, local } => write!(
                f,
                "the archive lists {source} path(s) for it, the recipe here {local}, so they cannot be matched"
            ),
            Self::NoSlot(at) => write!(f, "{at} lies under none of the item's paths"),
        }
    }
}

impl<'a> Planner<'a> {
    /// The OS the archive was captured on, when it is not this one.
    fn foreign_os(&self) -> Option<OsKind> {
        let from = self.manifest.source.os;
        (from != self.platform.os()).then_some(from)
    }

    /// Whether item `item` may move between OS families: the archive and
    /// the local recipe item (if any) must both say so.
    fn portable(&self, item: &ManifestItem) -> bool {
        item.flags.portability == Portability::CrossOs
            && self
                .local_item(item)
                .is_none_or(|li| li.flags.portability == carrova_recipe::Portability::CrossOs)
    }

    /// Rebase every cross-OS item of a foreign archive that the user did
    /// not remap (an explicit remap replaces the automatic mapping). Items
    /// that cannot be rebased are recorded with why.
    pub(super) fn rebase_items(&mut self) {
        if self.foreign_os().is_none() {
            return;
        }
        let manifest = self.manifest;
        for (item, key) in manifest.items.iter().zip(&self.keys) {
            if !self.portable(item) || self.options.remaps.contains_key(key) {
                continue;
            }
            match self.rebase(key, item) {
                Ok(r) => {
                    self.rebased.insert(key.clone(), r);
                }
                Err(why) => {
                    self.unmapped.insert(key.clone(), why.to_string());
                }
            }
        }
    }

    fn rebase(&self, key: &ItemKey, item: &ManifestItem) -> Result<Rebased, Unmapped> {
        let local = self
            .local_item(item)
            .ok_or(Unmapped::NoRecipe)?
            .paths_for(self.platform.os());
        let source = &item.source_paths;
        if source.len() != local.len() || source.is_empty() {
            return Err(Unmapped::Misaligned {
                source: source.len(),
                local: local.len(),
            });
        }
        let entries = item
            .entries
            .iter()
            .map(|e| {
                let (slot, rest) = slot_of(e, source.iter().map(|s| (s.root, s.rel.as_str())))
                    .ok_or_else(|| {
                        Unmapped::NoSlot(RootedPath::new(e.root, e.rel.as_str()).to_string())
                    })?;
                let to = &local[slot];
                Ok((to.root, format!("{}{rest}", to.rel.as_str())))
            })
            .collect::<Result<_, _>>()?;
        let slots = source
            .iter()
            .zip(local)
            .map(|(s, l)| PathMapping {
                item: key.clone(),
                from: (s.root, s.rel.as_str().to_owned()),
                to: (l.root, l.rel.as_str().to_owned()),
                to_abs: None,
            })
            .collect();
        Ok(Rebased { entries, slots })
    }

    /// Where entry `index` of item `key` lands before any user remap: its
    /// slot on this OS for a rebased item, else its own path.
    pub(super) fn local_path<'e>(
        &'e self,
        key: &ItemKey,
        index: usize,
        entry: &'e Entry,
    ) -> RootedPath<'e> {
        match self.rebased.get(key).and_then(|r| r.entries.get(index)) {
            Some((root, rel)) => RootedPath::new(*root, rel),
            None => RootedPath::new(entry.root, entry.rel.as_str()),
        }
    }

    /// Drop the selected items a foreign archive cannot restore here (one
    /// warning each); block same-OS items unless the user allowed them.
    pub(super) fn check_cross_os(&mut self) {
        let Some(from) = self.foreign_os() else {
            return;
        };
        let to = self.platform.os();
        let mut kept = Vec::with_capacity(self.selected.len());
        for idx in std::mem::take(&mut self.selected) {
            let key = self.keys[idx].clone();
            if let Some(why) = self.unmapped.get(&key).cloned() {
                self.warn(Some(&key), why);
                continue;
            }
            kept.push(idx);
            if self.portable(&self.manifest.items[idx]) {
                continue;
            }
            if self.options.allow_cross_os.contains(&key) {
                self.warn(
                    Some(&key),
                    format!(
                        "override: restoring a same-OS item from {from} on {to} at the same paths; the app may not read it here"
                    ),
                );
            } else {
                self.blockers.push(Blocker::CrossOs {
                    item: key,
                    from,
                    to,
                });
            }
        }
        self.selected = kept;
    }

    /// Mappings of the selected rebased items, item and slot order.
    pub(super) fn mappings(&self) -> Vec<PathMapping> {
        self.selected
            .iter()
            .filter_map(|&i| self.rebased.get(&self.keys[i]))
            .flat_map(|r| r.slots.iter().cloned())
            .map(|mut m| {
                m.to_abs = self.local_abs(m.to.0, &m.to.1);
                m
            })
            .collect()
    }
}

/// The slot (index into `slots`) holding `entry` — the deepest one when
/// slots nest — and the entry's remainder below it (`""` or `/…`).
fn slot_of<'s>(
    entry: &Entry,
    slots: impl Iterator<Item = (SemanticRoot, &'s str)>,
) -> Option<(usize, String)> {
    let rel = entry.rel.as_str();
    slots
        .enumerate()
        .filter(|(_, (root, base))| {
            RootedPath::new(entry.root, rel).is_within(RootedPath::new(*root, base))
        })
        .max_by_key(|(_, (_, base))| base.len())
        .map(|(i, (_, base))| (i, rel[base.len()..].to_owned()))
}
