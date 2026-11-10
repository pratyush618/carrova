//! OS-neutral lexical path helpers shared by every crate (one
//! implementation; callers never re-implement normalisation).

use std::path::{Component, Path, PathBuf};

/// Lexically normalise an absolute path: drop `.`, resolve `..` textually
/// (symlinks are never consulted). `None` if `path` is relative, has a
/// Windows prefix, or climbs above its root.
#[must_use]
pub fn normalize_abs(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for c in path.components() {
        match c {
            Component::RootDir => out.push(c),
            Component::CurDir => {}
            Component::ParentDir => {
                depth = depth.checked_sub(1)?;
                out.pop();
            }
            Component::Normal(n) => {
                out.push(n);
                depth += 1;
            }
            Component::Prefix(_) => return None,
        }
    }
    out.has_root().then_some(out)
}
