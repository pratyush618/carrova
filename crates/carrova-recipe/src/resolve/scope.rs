//! Checking an item path before it is walked.

use std::path::{Path, PathBuf};

use carrova_platform::FileKind;

use super::paths::{is_fs_root, normalize};
use super::{Lstat, Resolver, SkipReason};
use crate::PathRef;

/// A checked item path: its root, the path itself, and where symlinks
/// under it may point.
pub(super) struct Scope {
    pub(super) root_path: PathBuf,
    pub(super) target: PathBuf,
    pub(super) allowed: Vec<PathBuf>,
}

impl Resolver<'_> {
    /// Where `pr` lives and may link to, after checking that its root is
    /// usable, it stays inside the root and no parent component is a
    /// symlink. `None` once it is recorded as skipped or missing.
    pub(super) fn scope(&mut self, pr: &PathRef) -> Option<Scope> {
        let rel = pr.rel.as_str();
        let root_path = match self.platform.resolve_root(pr.root) {
            Ok(p) => p,
            Err(e) => {
                self.skip(pr.root, rel, SkipReason::RootUnavailable(e.to_string()));
                return None;
            }
        };
        let target = pr.rel.join_to(&root_path);
        let root_norm = normalize(&root_path);
        if is_fs_root(&root_norm) {
            // Defence in depth: adapters must never hand out `/`, but if one
            // does, confinement to that root would mean nothing.
            let why = format!("`{}` resolves to the filesystem root", pr.root);
            self.skip(pr.root, rel, SkipReason::RootUnavailable(why));
            return None;
        }
        let target_norm = normalize(&target);
        if target_norm == root_norm || !target_norm.starts_with(&root_norm) {
            self.skip(pr.root, rel, SkipReason::OutsideRoot);
            return None;
        }
        if !self.parents_are_dirs(pr, &root_path) || !self.target_exists(pr, &target) {
            return None;
        }
        // A symlink may point into its own item's root or into home, nowhere
        // else: trusting every resolved root would let one misconfigured
        // root widen confinement for all items.
        let allowed = std::iter::once(root_norm)
            .chain(self.home.clone())
            .collect();
        Some(Scope {
            root_path,
            target,
            allowed,
        })
    }

    /// Parent components must be real directories: a symlinked parent
    /// would mean following a link. `false` once `pr` is recorded as
    /// skipped or missing.
    fn parents_are_dirs(&mut self, pr: &PathRef, root_path: &Path) -> bool {
        let rel = pr.rel.as_str();
        let comps: Vec<&str> = pr.rel.components().collect();
        let mut cur = root_path.to_path_buf();
        for (i, c) in comps.iter().enumerate().take(comps.len().saturating_sub(1)) {
            cur.push(c);
            match self.lstat(&cur) {
                Lstat::Found(m) if m.kind == FileKind::Symlink => {
                    let ancestor = comps[..=i].join("/");
                    self.skip(pr.root, rel, SkipReason::SymlinkInPath { ancestor });
                    return false;
                }
                Lstat::Found(_) => {}
                Lstat::Missing => {
                    self.missing.push(pr.clone());
                    return false;
                }
                Lstat::Failed(e) => {
                    self.skip(pr.root, rel, SkipReason::Io(e));
                    return false;
                }
            }
        }
        true
    }

    /// Whether the item path itself exists; otherwise it is recorded as
    /// missing or skipped.
    fn target_exists(&mut self, pr: &PathRef, target: &Path) -> bool {
        let rel = pr.rel.as_str();
        match self.lstat(target) {
            Lstat::Found(_) => {}
            Lstat::Missing => {
                self.missing.push(pr.clone());
                return false;
            }
            Lstat::Failed(e) => {
                self.skip(pr.root, rel, SkipReason::Io(e));
                return false;
            }
        }
        true
    }
}
