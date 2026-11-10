//! OS-neutral lexical path helpers shared by every crate (one
//! implementation; callers never re-implement normalisation).

use std::path::{Component, Path, PathBuf, Prefix};

/// Lexically normalise an absolute path: drop `.`, resolve `..` textually
/// (symlinks are never consulted). `None` if `path` is not absolute or
/// climbs above its root.
///
/// A Windows drive (`C:\`) or UNC (`\\server\share\`) prefix is kept and
/// must be followed by a root, so drive-relative `C:foo` is refused.
/// Verbatim (`\\?\`) and device (`\\.\`) prefixes are refused: the OS
/// takes their `..` literally, so lexical resolution would disagree with
/// it.
#[must_use]
pub fn normalize_abs(path: &Path) -> Option<PathBuf> {
    let mut out = PathBuf::new();
    let mut depth = 0usize;
    for c in path.components() {
        match c {
            Component::Prefix(p) if is_supported_prefix(p.kind()) => out.push(c),
            Component::Prefix(_) => return None,
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
        }
    }
    out.has_root().then_some(out)
}

/// Windows prefixes whose paths resolve `..` the way [`normalize_abs`]
/// does: drive letters and UNC shares.
fn is_supported_prefix(prefix: Prefix<'_>) -> bool {
    matches!(prefix, Prefix::Disk(_) | Prefix::UNC(..))
}

#[cfg(test)]
#[path = "../tests/path/unit.rs"]
mod tests;
