//! Redaction list of a session (`redact.json`): every form in which a
//! sensitive item's paths can appear in the log, so
//! [`super::sessions::export_log`] can hide them.

use std::collections::BTreeSet;
use std::path::Path;

use carrova_manifest::Manifest;
use carrova_platform::{Platform, SemanticRoot};

use super::session::Session;
use crate::error::Result;
use crate::key::{ItemKey, RootedPath};
use crate::plan::{Action, Plan};
use crate::select::Selection;

/// Redaction patterns of a session (JSON list of strings).
pub(super) const REDACT_FILE: &str = "redact.json";

/// Patterns being collected: `<root>:<rel>` keys and absolute paths.
pub(super) struct Patterns<'a> {
    platform: &'a dyn Platform,
    set: BTreeSet<String>,
}

impl<'a> Patterns<'a> {
    pub(super) fn new(platform: &'a dyn Platform) -> Self {
        Self {
            platform,
            set: BTreeSet::new(),
        }
    }

    /// A location as its key and (if the root resolves) absolute path.
    pub(super) fn location(&mut self, root: SemanticRoot, rel: &str) {
        self.set.insert(RootedPath::new(root, rel).to_string());
        if let Ok(base) = self.platform.resolve_root(root) {
            self.set.insert(base.join(rel).display().to_string());
        }
    }

    /// An absolute path.
    pub(super) fn path(&mut self, abs: &Path) {
        self.set.insert(abs.display().to_string());
    }

    /// Paths of the sensitive items of a sender's selection.
    pub(super) fn selection(mut self, selection: &Selection) -> Self {
        for c in selection.items.values().filter(|c| c.sensitive) {
            for p in c.paths.iter().filter_map(|p| RootedPath::parse(p)) {
                self.location(p.root, p.rel);
            }
        }
        self
    }

    /// Entry paths of the items an archive or sender flags sensitive.
    pub(super) fn manifest(mut self, manifest: &Manifest) -> Self {
        for item in manifest.items.iter().filter(|i| i.flags.sensitive) {
            for e in &item.entries {
                self.location(e.root, e.rel.as_str());
            }
        }
        self
    }

    /// Receiver targets (incl. keep-both names and handler backups) of the
    /// items that are sensitive by the manifest or the receiver's own
    /// rules ([`Plan::sensitive`]).
    pub(super) fn plan(mut self, plan: &Plan, manifest: Option<&Manifest>) -> Self {
        let keys: BTreeSet<ItemKey> = manifest
            .iter()
            .flat_map(|m| m.items.iter())
            .filter(|i| i.flags.sensitive)
            .map(ItemKey::of)
            .chain(plan.sensitive.keys().cloned())
            .collect();
        for a in plan.actions.iter().filter(|a| keys.contains(&a.item)) {
            self.location(a.root, &a.rel);
            self.path(&a.abs);
            if let Action::KeepBoth { rel } = &a.action {
                self.location(a.root, rel);
            }
        }
        for h in plan.handlers.iter().filter(|h| keys.contains(&h.item)) {
            for (root, rel) in &h.backup {
                self.location(*root, rel);
            }
        }
        self
    }
}

impl Session {
    /// Merge `patterns` into the persisted redaction list.
    pub(super) fn add_redactions(&self, patterns: Patterns<'_>) -> Result<()> {
        let mut all: BTreeSet<String> = self.read_json(REDACT_FILE)?.unwrap_or_default();
        let before = all.len();
        all.extend(patterns.set.into_iter().filter(|p| !p.is_empty()));
        if all.len() != before || before == 0 {
            self.write_json(REDACT_FILE, &all)?;
        }
        Ok(())
    }
}
