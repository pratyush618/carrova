//! Pass 1: the final (remapped) target of every selected entry.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use carrova_manifest::{Entry, EntryKind, ManifestItem, RelPath};
use carrova_platform::{FileKind, SemanticRoot};

use super::Planner;
use super::skip::SkipReason;
use crate::error::{CoreError, Result};
use crate::key::ItemKey;
use crate::plan::{Action, ObjectKind, PlannedAction};

/// One incoming entry with its final (remapped) target.
pub(super) struct Target<'m> {
    pub(super) key: ItemKey,
    pub(super) entry_index: usize,
    pub(super) entry: &'m Entry,
    pub(super) root: SemanticRoot,
    pub(super) rel: String,
    /// `None` when the root could not be resolved.
    pub(super) abs: Option<PathBuf>,
    /// Decided before inspecting the receiver (root unusable, duplicate, …).
    pub(super) pre_skip: Option<SkipReason>,
    /// Final mode apply sets (see [`PlannedAction::mode`]).
    pub(super) mode: Option<u32>,
}

impl Target<'_> {
    /// Nothing is written below it once it is in place.
    pub(super) fn is_incoming_non_dir(&self) -> bool {
        self.pre_skip.is_none() && !matches!(self.entry.kind, EntryKind::Dir)
    }

    /// The planned action for this target.
    pub(super) fn planned(&self, action: Action) -> PlannedAction {
        let (kind, size, link_target) = match &self.entry.kind {
            EntryKind::File { size, .. } => (ObjectKind::File, *size, None),
            EntryKind::Dir => (ObjectKind::Dir, 0, None),
            EntryKind::Symlink { target } => (ObjectKind::Symlink, 0, Some(target.clone())),
        };
        PlannedAction {
            item: self.key.clone(),
            entry_index: self.entry_index,
            kind,
            root: self.root,
            rel: self.rel.clone(),
            abs: self.abs.clone().unwrap_or_default(),
            size,
            mode: self.mode,
            link_target,
            mtime_ns: self.entry.mtime_ns,
            action,
        }
    }
}

impl<'a> Planner<'a> {
    /// Final target of every selected entry (remaps applied).
    pub(super) fn targets(&mut self) -> Result<Vec<Target<'a>>> {
        let mut out = Vec::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let manifest = self.manifest;
        for idx in self.selected.clone() {
            let item = &manifest.items[idx];
            let key = self.keys[idx].clone();
            self.warn_remap(&key, item);
            let handler_skip = item
                .handler
                .as_deref()
                .filter(|h| !crate::handlers::is_known(h))
                .map(|h| SkipReason::UnknownHandler(h.to_owned()));
            for (entry_index, entry) in item.entries.iter().enumerate() {
                let mut t = self.target(&key, entry_index, entry)?;
                if t.pre_skip.is_none() {
                    t.pre_skip.clone_from(&handler_skip);
                }
                if t.pre_skip.is_none()
                    && let Some(a) = &t.abs
                    && !seen.insert(a.clone())
                {
                    t.pre_skip = Some(SkipReason::DuplicateTarget);
                }
                out.push(t);
            }
        }
        Ok(out)
    }

    /// One entry's target: remapped, re-validated, resolved under its root.
    fn target(&self, key: &ItemKey, entry_index: usize, entry: &'a Entry) -> Result<Target<'a>> {
        let (root, rel) = self.remapped(key, entry);
        // Re-validate the composed path (defence in depth).
        RelPath::new(rel.as_str()).map_err(|e| CoreError::UnsafePath {
            path: rel.clone(),
            reason: e.to_string(),
        })?;
        let (abs, pre_skip) = match self.roots.usable(root) {
            Some(Ok(base)) => {
                let mut abs = base.clone();
                abs.extend(rel.split('/'));
                (Some(abs), None)
            }
            Some(Err(unusable)) => (None, Some(SkipReason::Root(unusable.clone()))),
            None => (None, Some(SkipReason::RootUnavailable)),
        };
        let mode = self.final_mode(entry, abs.as_deref());
        Ok(Target {
            key: key.to_owned(),
            entry_index,
            entry,
            root,
            rel,
            abs,
            pre_skip,
            mode,
        })
    }

    /// Tell the user an item is restored elsewhere (and that its handler
    /// step is not remapped).
    fn warn_remap(&mut self, key: &ItemKey, item: &ManifestItem) {
        let Some(remap) = self.options.remaps.get(key) else {
            return;
        };
        let root = remap
            .root
            .map_or_else(|| "(unchanged)".into(), |r| r.to_string());
        let prefix = self.prefixes.get(key).map_or("(none)", RelPath::as_str);
        self.warn(
            Some(key),
            format!(
                "override: restoring to a different location (remap root {root}, prefix {prefix})"
            ),
        );
        if item.handler.is_some() {
            self.warn(
                Some(key),
                "remap does not apply to the item's handler step".into(),
            );
        }
    }

    /// Final `(root, rel)` of an entry after the item's remap.
    pub(super) fn remapped(&self, key: &ItemKey, entry: &Entry) -> (SemanticRoot, String) {
        let root = self
            .options
            .remaps
            .get(key)
            .and_then(|r| r.root)
            .unwrap_or(entry.root);
        let rel = match self.prefixes.get(key) {
            Some(p) => format!("{}/{}", p.as_str(), entry.rel.as_str()),
            None => entry.rel.as_str().to_owned(),
        };
        (root, rel)
    }

    /// Final mode for an entry under the plan's mode policy (an existing
    /// directory at `abs` is only ever tightened).
    fn final_mode(&self, entry: &Entry, abs: Option<&Path>) -> Option<u32> {
        let incoming = entry.mode?;
        let existing_dir = match (&entry.kind, abs.map(|a| self.probe(a))) {
            (EntryKind::Dir, Some(Ok(Some(m)))) if m.kind == FileKind::Dir => m.mode,
            _ => None,
        };
        Some(self.policy.final_mode(incoming, existing_dir))
    }
}
