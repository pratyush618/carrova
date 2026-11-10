//! Pass 2: the action for every target, checked against the receiver
//! (lstat walks, never through a symlink), plus the per-item mode warnings.
//!
//! Action semantics apply relies on: `Replace` on a directory only sets
//! its mode (dir ↔ non-dir conflicts are skipped); `KeepBoth { rel }` is a
//! sibling of the original target.

use std::collections::{BTreeMap, HashSet};
use std::io::BufReader;
use std::path::{Path, PathBuf};

use carrova_manifest::{Blake3Hash, DEFAULT_CHUNK_PARAMS, EntryKind, RelPath, chunk_reader};
use carrova_platform::path::normalize_abs;
use carrova_platform::{FileKind, FileMeta};

use super::Planner;
use super::skip::{Decision, SkipReason};
use super::targets::Target;
use crate::key::{ItemKey, RootedPath};
use crate::mode::DroppedBits;
use crate::plan::confine::{link_destination, symlink_escapes};
use crate::plan::{Action, ConflictRule, KEEP_BOTH_MAX_TRIES, PlannedAction};

/// Paths claimed by the plan so far.
struct Occupancy {
    /// Occupied by an incoming non-directory: nothing may be written below.
    incoming_non_dirs: HashSet<PathBuf>,
    /// Some selected entry or keep-both name occupies it.
    taken: HashSet<PathBuf>,
    /// Per-file rules that matched a conflict.
    used_path_rules: HashSet<String>,
}

impl Occupancy {
    fn of(targets: &[Target<'_>]) -> Self {
        Self {
            incoming_non_dirs: targets
                .iter()
                .filter(|t| t.is_incoming_non_dir())
                .filter_map(|t| t.abs.clone())
                .collect(),
            taken: targets.iter().filter_map(|t| t.abs.clone()).collect(),
            used_path_rules: HashSet::new(),
        }
    }
}

impl Planner<'_> {
    /// Decide every target (in order) and warn about every skip the user
    /// did not choose, dropped mode bits and unused per-file rules.
    pub(super) fn actions(&mut self, targets: &[Target<'_>]) -> Vec<PlannedAction> {
        if self.policy.keep_group_write {
            self.warn(
                None,
                "override: keeping group-write bits from the archive".into(),
            );
        }
        let mut occupancy = Occupancy::of(targets);
        let mut dropped: BTreeMap<ItemKey, DroppedBits> = BTreeMap::new();
        let mut actions = Vec::with_capacity(targets.len());
        for t in targets {
            let decision = match &t.pre_skip {
                Some(reason) => Decision::Skip(reason.clone()),
                None => self.decide(t, &mut occupancy),
            };
            let action = self.resolve(t, decision);
            if let (Some(from), Some(to)) = (t.entry.mode, t.mode)
                && !matches!(action, Action::Skip { .. })
                && !matches!(t.entry.kind, EntryKind::Symlink { .. })
            {
                dropped.entry(t.key.clone()).or_default().add(from, to);
            }
            actions.push(t.planned(action));
        }
        self.warn_dropped_modes(dropped);
        self.warn_unused_path_rules(&occupancy.used_path_rules);
        actions
    }

    /// The action for a decision; a skip the user did not choose is also
    /// a warning.
    fn resolve(&mut self, t: &Target<'_>, decision: Decision) -> Action {
        match decision {
            Decision::Write(action) => action,
            Decision::Skip(reason) => {
                if !reason.chosen_by_user() {
                    let msg = format!("skipped {}: {reason}", RootedPath::new(t.root, &t.rel));
                    self.warn(Some(&t.key), msg);
                }
                Action::skip(reason.to_string())
            }
        }
    }

    /// Action for one target not skipped up front.
    fn decide(&self, t: &Target<'_>, occupancy: &mut Occupancy) -> Decision {
        let (Some(abs), Some(Ok(root_abs))) = (&t.abs, self.roots.usable(t.root)) else {
            return Decision::Skip(SkipReason::RootUnavailable);
        };
        let absent = match self.walk_ancestors(t, root_abs, &occupancy.incoming_non_dirs) {
            Ok(absent) => absent,
            Err(reason) => return Decision::Skip(reason),
        };
        if let EntryKind::Symlink { target } = &t.entry.kind
            && let Some(reason) = self.symlink_refusal(target, abs, root_abs)
        {
            return Decision::Skip(reason);
        }
        if absent {
            return Decision::Write(Action::Create);
        }
        let existing = match self.probe(abs) {
            Ok(None) => return Decision::Write(Action::Create),
            Ok(Some(m)) => m,
            Err(e) => return Decision::Skip(SkipReason::TargetUninspectable(e)),
        };
        match self.matches_existing(t, abs, &existing) {
            Ok(true) => Decision::Write(Action::Identical),
            Ok(false) => self.conflict_action(t, abs, occupancy),
            Err(reason) => Decision::Skip(reason),
        }
    }

    /// Walk the target's ancestors root → parent with lstat: never write
    /// through a symlink or below a non-directory (existing or incoming).
    /// `Ok(true)` once an ancestor is absent (the target is too).
    fn walk_ancestors(
        &self,
        t: &Target<'_>,
        root_abs: &Path,
        incoming_non_dirs: &HashSet<PathBuf>,
    ) -> Result<bool, SkipReason> {
        let comps: Vec<&str> = t.rel.split('/').collect();
        let mut absent = false;
        let mut cur = root_abs.to_path_buf();
        for (i, c) in comps[..comps.len() - 1].iter().enumerate() {
            cur.push(c);
            let shown = || RootedPath::new(t.root, &comps[..=i].join("/")).to_string();
            if incoming_non_dirs.contains(&cur) {
                return Err(SkipReason::IncomingAncestor(shown()));
            }
            if absent {
                continue;
            }
            match self.probe(&cur) {
                Ok(None) => absent = true,
                Ok(Some(m)) => match m.kind {
                    FileKind::Dir => {}
                    FileKind::Symlink => return Err(SkipReason::SymlinkAncestor(shown())),
                    FileKind::File | FileKind::Other => {
                        return Err(SkipReason::FileAncestor(shown()));
                    }
                },
                Err(e) => return Err(SkipReason::AncestorUninspectable(shown(), e)),
            }
        }
        Ok(absent)
    }

    /// Whether the existing object at `abs` already is the incoming one
    /// (same kind, content, mode); a kind clash is a skip.
    fn matches_existing(
        &self,
        t: &Target<'_>,
        abs: &Path,
        existing: &FileMeta,
    ) -> Result<bool, SkipReason> {
        let mode_eq = t.mode.is_none() || existing.mode.map(|m| m & 0o7777) == t.mode;
        match (&t.entry.kind, existing.kind) {
            (_, FileKind::Other) => Err(SkipReason::SpecialFile),
            (EntryKind::Dir, FileKind::Dir) => Ok(mode_eq),
            (EntryKind::Dir, _) => Err(SkipReason::DirOverNonDir),
            (_, FileKind::Dir) => Err(SkipReason::NonDirOverDir),
            (EntryKind::File { size, hash, .. }, FileKind::File) => {
                Ok(mode_eq && existing.size == *size && self.hash_existing(abs) == Some(*hash))
            }
            (EntryKind::Symlink { target }, FileKind::Symlink) => Ok(self
                .platform
                .read_link(abs)
                .ok()
                .is_some_and(|p| p.as_os_str() == std::ffi::OsStr::new(target))),
            (EntryKind::File { .. } | EntryKind::Symlink { .. }, _) => Ok(false),
        }
    }

    /// Conflict: per-file rule > per-item rule > default.
    fn conflict_action(&self, t: &Target<'_>, abs: &Path, occupancy: &mut Occupancy) -> Decision {
        let pk = RootedPath::new(t.root, &t.rel).to_string();
        let rule = match self.options.path_rules.get(&pk) {
            Some(rule) => {
                occupancy.used_path_rules.insert(pk);
                *rule
            }
            None => self.options.item_rule(&t.key),
        };
        match rule {
            ConflictRule::Replace => Decision::Write(Action::Replace),
            ConflictRule::Skip => Decision::Skip(SkipReason::RuleSkip),
            ConflictRule::KeepBoth if matches!(t.entry.kind, EntryKind::Dir) => {
                Decision::Skip(SkipReason::KeepBothDir)
            }
            ConflictRule::KeepBoth => self.keep_both(t, abs, &mut occupancy.taken),
        }
    }

    /// First free `<name>.<suffix>`, `<name>.<suffix>-2`, … next to `abs`.
    fn keep_both(&self, t: &Target<'_>, abs: &Path, taken: &mut HashSet<PathBuf>) -> Decision {
        let Some(parent) = abs.parent() else {
            return Decision::Skip(SkipReason::NoParent);
        };
        let (parent_rel, name) = t.rel.rsplit_once('/').unwrap_or(("", &t.rel));
        for n in 1..=KEEP_BOTH_MAX_TRIES {
            let candidate = if n == 1 {
                format!("{name}.{}", self.suffix)
            } else {
                format!("{name}.{}-{n}", self.suffix)
            };
            let cand_abs = parent.join(&candidate);
            if taken.contains(&cand_abs) || !matches!(self.probe(&cand_abs), Ok(None)) {
                continue;
            }
            let rel = if parent_rel.is_empty() {
                candidate
            } else {
                format!("{parent_rel}/{candidate}")
            };
            if RelPath::new(rel.as_str()).is_err() {
                break;
            }
            taken.insert(cand_abs);
            return Decision::Write(Action::KeepBoth { rel });
        }
        Decision::Skip(SkipReason::NoKeepBothName)
    }

    /// Why a symlink must not be created: its target escapes the target
    /// root and home, or goes through a link that exists on this machine.
    fn symlink_refusal(&self, target: &str, link: &Path, root_abs: &Path) -> Option<SkipReason> {
        if symlink_escapes(target, link, root_abs, self.roots.home()) {
            return Some(SkipReason::SymlinkEscapes(target.to_owned()));
        }
        self.symlink_through_link(target, link, root_abs)
    }

    /// lstat-walks the existing part of a (confined) symlink target below
    /// the root (or home) it is in: refused if it passes through a link.
    fn symlink_through_link(
        &self,
        target: &str,
        link: &Path,
        root_abs: &Path,
    ) -> Option<SkipReason> {
        let resolved = normalize_abs(&link_destination(target, link)?)?;
        let base = [Some(root_abs), self.roots.home()]
            .into_iter()
            .flatten()
            .filter(|b| resolved.starts_with(b))
            .max_by_key(|b| b.components().count())?;
        let rest = resolved.strip_prefix(base).ok()?;
        let mut cur = base.to_path_buf();
        for c in rest.components() {
            cur.push(c);
            match self.probe(&cur) {
                Ok(None) => return None,
                Ok(Some(m)) if m.kind == FileKind::Symlink => {
                    return Some(SkipReason::SymlinkThroughLink(target.to_owned()));
                }
                Ok(Some(_)) => {}
                Err(e) => {
                    return Some(SkipReason::SymlinkTargetUninspectable(target.to_owned(), e));
                }
            }
        }
        None
    }

    /// BLAKE3 of an existing regular file (nofollow); `None` if unreadable.
    fn hash_existing(&self, abs: &Path) -> Option<Blake3Hash> {
        let file = self.platform.open_read_nofollow(abs).ok()?;
        chunk_reader(BufReader::new(file), DEFAULT_CHUNK_PARAMS)
            .ok()
            .map(|fc| fc.file_hash)
    }

    /// One warning per item whose archive modes lost bits.
    fn warn_dropped_modes(&mut self, dropped: BTreeMap<ItemKey, DroppedBits>) {
        for (item, d) in dropped.into_iter().filter(|(_, d)| d.paths > 0) {
            let hint = if d.group_write_only() && !self.policy.keep_group_write {
                " (keep with --keep-group-write)"
            } else {
                ""
            };
            let msg = format!(
                "{} dropped on {} path(s){hint}",
                d.classes().join(", "),
                d.paths
            );
            self.warn(Some(&item), msg);
        }
    }

    /// Per-file rules that matched no conflicting target.
    fn warn_unused_path_rules(&mut self, used: &HashSet<String>) {
        let options = self.options;
        for k in options.path_rules.keys() {
            if !used.contains(k.as_str()) {
                self.warn(
                    None,
                    format!("per-file rule for `{k}` matches no conflicting target"),
                );
            }
        }
    }
}
