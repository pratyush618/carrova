//! Unrecognised data: directories under `user_config` / `user_data` that no
//! detected recipe item claims.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use carrova_platform::SemanticRoot;
use tracing::debug;
use walkdir::WalkDir;

use super::{RawItem, ScanError};
use crate::cancel::Cancel;

/// Immediate child directories of `user_config` / `user_data` not claimed by
/// any detected item path. A child is claimed when an item path equals it,
/// lies inside it, or contains it (compared as absolute paths, so items under
/// other roots, e.g. `home:.config/x`, claim too). Children that contain
/// another semantic root are skipped so nothing is counted twice. `cancel`
/// is checked before every entry walked.
pub(super) fn find(
    roots: &BTreeMap<SemanticRoot, PathBuf>,
    claims: &[PathBuf],
    cancel: &Cancel,
    warnings: &mut Vec<String>,
) -> Result<Vec<RawItem>, ScanError> {
    let mut out = Vec::new();
    for root in [SemanticRoot::UserConfig, SemanticRoot::UserData] {
        let Some(base) = roots.get(&root) else {
            continue;
        };
        for (rel, path) in child_dirs(root, base, warnings) {
            let claimed = claims
                .iter()
                .any(|c| c.starts_with(&path) || path.starts_with(c));
            let holds_root = roots.values().any(|r| r != base && r.starts_with(&path));
            if claimed || holds_root {
                continue;
            }
            let (size, file_count, unreadable) = measure(&path, cancel)?;
            out.push(RawItem {
                root,
                rel,
                size,
                file_count,
                unreadable,
            });
        }
    }
    debug!(raw = out.len(), "unrecognised scan done");
    Ok(out)
}

/// Immediate child directories (never symlinks) of `root` at `base`, as
/// `(name, path)` sorted by name. Listing problems become warnings.
fn child_dirs(
    root: SemanticRoot,
    base: &Path,
    warnings: &mut Vec<String>,
) -> Vec<(String, PathBuf)> {
    let listing = match std::fs::read_dir(base) {
        Ok(l) => l,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Vec::new(),
        Err(e) => {
            warnings.push(format!("cannot list `{root}`: {e}"));
            return Vec::new();
        }
    };
    let mut children: Vec<(String, PathBuf)> = Vec::new();
    let mut unlisted = 0u64;
    for entry in listing {
        let Ok(entry) = entry else {
            unlisted += 1;
            continue;
        };
        // `DirEntry::file_type` does not follow symlinks.
        if entry.file_type().is_ok_and(|t| t.is_dir()) {
            children.push((
                entry.file_name().to_string_lossy().into_owned(),
                entry.path(),
            ));
        }
    }
    if unlisted > 0 {
        warnings.push(format!("{unlisted} unreadable entries in `{root}`"));
    }
    children.sort();
    children
}

/// `(bytes, regular files, unreadable entries)` under `dir`, never following
/// symlinks; [`ScanError::Cancelled`] once `cancel` fires.
fn measure(dir: &Path, cancel: &Cancel) -> Result<(u64, u64, u64), ScanError> {
    let (mut size, mut files, mut unreadable) = (0u64, 0u64, 0u64);
    for entry in WalkDir::new(dir).follow_links(false) {
        ScanError::check(cancel)?;
        match entry {
            Ok(e) if e.file_type().is_file() => match e.metadata() {
                Ok(m) => {
                    size += m.len();
                    files += 1;
                }
                Err(_) => unreadable += 1,
            },
            Ok(_) => {}
            Err(_) => unreadable += 1,
        }
    }
    Ok((size, files, unreadable))
}

#[cfg(test)]
#[path = "../../tests/scan/unrecognised/unit.rs"]
mod tests;
