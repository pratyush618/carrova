//! Lexical path helpers (no filesystem access).

use std::path::{Component, Path, PathBuf};

/// `path` relative to `base` as a `/`-joined string. `Err(lossy)` if a
/// component is not UTF-8.
pub(super) fn rel_string(path: &Path, base: &Path) -> Result<String, String> {
    let stripped = path.strip_prefix(base).unwrap_or(path);
    let mut parts = Vec::new();
    let mut lossy = false;
    for c in stripped.components() {
        let os = c.as_os_str();
        match os.to_str() {
            Some(s) => parts.push(s.to_owned()),
            None => {
                lossy = true;
                parts.push(os.to_string_lossy().into_owned());
            }
        }
    }
    let s = parts.join("/");
    if lossy { Err(s) } else { Ok(s) }
}

/// True if `p` (already normalised) has no normal components, i.e. is `/`,
/// empty, or only `..`s.
pub(super) fn is_fs_root(p: &Path) -> bool {
    !p.components().any(|c| matches!(c, Component::Normal(_)))
}

/// Lexical normalisation: drop `.`, apply `..` (never above the root).
pub(super) fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !out.pop() && !out.has_root() {
                    out.push("..");
                }
            }
            other => out.push(other),
        }
    }
    out
}

#[cfg(test)]
#[path = "../../tests/resolve/paths/unit.rs"]
mod tests;
