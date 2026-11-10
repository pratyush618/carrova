//! Walking a checked item path and recording what it holds.

use std::path::{Path, PathBuf};

use carrova_platform::FileKind;
use globset::GlobSet;
use walkdir::WalkDir;

use super::paths::{normalize, rel_string};
use super::scope::Scope;
use super::{FileEntry, Lstat, Resolver, SkipReason};
use crate::PathRef;

impl Resolver<'_> {
    pub(super) fn path(&mut self, pr: &PathRef, excludes: &GlobSet) {
        let Some(scope) = self.scope(pr) else {
            return;
        };
        let mut walk = WalkDir::new(&scope.target)
            .follow_links(false)
            .follow_root_links(false)
            .sort_by_file_name()
            .into_iter();
        while let Some(next) = walk.next() {
            if self.visit(pr, &scope, excludes, next) == Visit::Prune {
                walk.skip_current_dir();
            }
        }
    }

    /// One walk result under `scope`: record it as an entry or a skip.
    fn visit(
        &mut self,
        pr: &PathRef,
        scope: &Scope,
        excludes: &GlobSet,
        next: walkdir::Result<walkdir::DirEntry>,
    ) -> Visit {
        let ent = match next {
            Ok(e) => e,
            Err(e) => {
                self.walk_failed(pr, scope, &e);
                return Visit::Next;
            }
        };
        let path = ent.path();
        let prune = if ent.file_type().is_dir() {
            Visit::Prune
        } else {
            Visit::Next
        };
        let rel_root = match rel_string(path, &scope.root_path) {
            Ok(s) => s,
            Err(lossy) => {
                self.skip(pr.root, &lossy, SkipReason::NonUtf8Name);
                return prune;
            }
        };
        let rel_item = rel_string(path, &scope.target).unwrap_or_default();
        if (!rel_item.is_empty() && excludes.is_match(&rel_item)) || excludes.is_match(&rel_root) {
            return prune;
        }
        self.record(pr, scope, path, rel_root);
        Visit::Next
    }

    /// A walk error under `scope`, recorded as an I/O skip.
    fn walk_failed(&mut self, pr: &PathRef, scope: &Scope, e: &walkdir::Error) {
        let at = e
            .path()
            .and_then(|p| rel_string(p, &scope.root_path).ok())
            .unwrap_or_else(|| pr.rel.as_str().to_owned());
        let msg = e
            .io_error()
            .map_or_else(|| e.to_string(), ToString::to_string);
        self.skip(pr.root, &at, SkipReason::Io(msg));
    }

    /// Record the object at `path` as an entry, or as a skip if it cannot
    /// move (special file, escaping symlink, I/O error).
    fn record(&mut self, pr: &PathRef, scope: &Scope, path: &Path, rel_root: String) {
        let meta = match self.lstat(path) {
            Lstat::Found(m) => m,
            Lstat::Missing => return, // vanished during the walk
            Lstat::Failed(e) => {
                self.skip(pr.root, &rel_root, SkipReason::Io(e));
                return;
            }
        };
        let link_target = match meta.kind {
            FileKind::File | FileKind::Dir => None,
            FileKind::Other => {
                self.skip(pr.root, &rel_root, SkipReason::SpecialFile);
                return;
            }
            FileKind::Symlink => match self.link_target(path, &scope.allowed) {
                Ok(t) => Some(t),
                Err(reason) => {
                    self.skip(pr.root, &rel_root, reason);
                    return;
                }
            },
        };
        self.out.insert(
            (pr.root, rel_root.clone()),
            FileEntry {
                root: pr.root,
                rel: rel_root,
                kind: meta.kind,
                size: meta.size,
                mode: meta.mode,
                link_target,
            },
        );
    }

    /// Target of the symlink at `path`, if it resolves inside `allowed`.
    fn link_target(&self, path: &Path, allowed: &[PathBuf]) -> Result<PathBuf, SkipReason> {
        let t = self
            .platform
            .read_link(path)
            .map_err(|e| SkipReason::Io(e.to_string()))?;
        let resolved = match path.parent() {
            Some(parent) => normalize(&parent.join(&t)),
            None => normalize(&t),
        };
        if allowed.iter().any(|r| resolved.starts_with(r)) {
            Ok(t)
        } else {
            Err(SkipReason::SymlinkEscapesRoots { target: t })
        }
    }
}

/// What the walk does after one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Visit {
    Next,
    /// Do not descend (an excluded or unnamed directory).
    Prune,
}
