//! Item resolution: which files would move for an item.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use carrova_platform::{FileKind, FileMeta, Platform, PlatformError, SemanticRoot};
use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use tracing::debug;

use crate::{Item, PathRef};

mod paths;
mod scope;
mod types;
mod walk;

use paths::{is_fs_root, normalize};
pub use types::{FileEntry, ResolvedItem, SkipReason, Skipped};

/// Compile exclude globs. `*`/`?` do not cross `/`; `**` does.
pub fn build_excludes(globs: &[String]) -> Result<GlobSet, globset::Error> {
    let mut b = GlobSetBuilder::new();
    for g in globs {
        b.add(GlobBuilder::new(g).literal_separator(true).build()?);
    }
    b.build()
}

/// List every file an item would move.
///
/// Uses the item's paths and excludes for the platform's OS
/// ([`Item::paths_for`], [`Item::exclude_for`]). For each `(root, rel)`
/// path: resolve the root, join `rel`, and walk it
/// without following any symlink. Symlinks become [`FileKind::Symlink`]
/// entries, unless their target (resolved lexically against the link's
/// directory) lies outside both the item path's own root and the home root,
/// in which case they are skipped. A root that resolves to the filesystem
/// root is treated as unavailable. A symlink in a *parent* component of `rel` skips the whole path.
/// Exclude globs are matched against the path relative to the item path and
/// relative to the semantic root; an excluded directory prunes its subtree.
///
/// Unmovable items resolve to nothing.
#[must_use]
pub fn resolve_item(item: &Item, platform: &dyn Platform) -> ResolvedItem {
    let mut r = Resolver {
        platform,
        out: BTreeMap::new(),
        missing: Vec::new(),
        skipped: Vec::new(),
        home: platform
            .resolve_root(SemanticRoot::Home)
            .ok()
            .map(|p| normalize(&p))
            .filter(|p| !is_fs_root(p)),
    };
    if item.is_movable() {
        let os = platform.os();
        let paths = item.paths_for(os);
        match build_excludes(item.exclude_for(os)) {
            Ok(excludes) => {
                for pr in paths {
                    r.path(pr, &excludes);
                }
            }
            Err(e) => {
                for pr in paths {
                    r.skip(
                        pr.root,
                        pr.rel.as_str(),
                        SkipReason::InvalidExclude(e.to_string()),
                    );
                }
            }
        }
    }
    let entries: Vec<FileEntry> = r.out.into_values().collect();
    let total_size = entries
        .iter()
        .filter(|e| e.kind == FileKind::File)
        .map(|e| e.size)
        .sum();
    r.skipped.sort();
    r.skipped.dedup();
    ResolvedItem {
        item_id: item.id.clone(),
        entries,
        missing: r.missing,
        skipped: r.skipped,
        total_size,
    }
}

struct Resolver<'a> {
    platform: &'a dyn Platform,
    out: BTreeMap<(SemanticRoot, String), FileEntry>,
    missing: Vec<PathRef>,
    skipped: Vec<Skipped>,
    /// Lexically normalised home root, if it resolved (and is not `/`).
    home: Option<PathBuf>,
}

enum Lstat {
    Found(FileMeta),
    Missing,
    Failed(String),
}

impl Resolver<'_> {
    fn skip(&mut self, root: SemanticRoot, rel: &str, reason: SkipReason) {
        debug!(%root, rel, %reason, "skipping");
        self.skipped.push(Skipped {
            root,
            rel: rel.to_owned(),
            reason,
        });
    }

    fn lstat(&self, path: &Path) -> Lstat {
        match self.platform.metadata(path) {
            Ok(m) => Lstat::Found(m),
            Err(PlatformError::Io { source, .. }) if source.kind() == io::ErrorKind::NotFound => {
                Lstat::Missing
            }
            Err(e) => Lstat::Failed(e.to_string()),
        }
    }
}
