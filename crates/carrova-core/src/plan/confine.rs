//! Confinement checks of the planner: lstat probes, usable roots, lexical
//! path normalisation and symlink-target containment. Read-only.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use carrova_platform::path::normalize_abs;
use carrova_platform::{FileKind, FileMeta, Platform, PlatformError, SemanticRoot};

/// A filesystem query of the planner failed (cheap to clone: probes are
/// cached and their outcome shown in several places).
#[derive(Debug, Clone, thiserror::Error)]
#[error(transparent)]
pub(super) struct ProbeError(Arc<PlatformError>);

impl From<PlatformError> for ProbeError {
    fn from(e: PlatformError) -> Self {
        Self(Arc::new(e))
    }
}

/// lstat of `path`: `Ok(None)` = absent, `Err` = cannot inspect.
pub(super) type Probe = Result<Option<FileMeta>, ProbeError>;

/// lstat `path` (never follows a final symlink).
pub(super) fn probe(platform: &dyn Platform, path: &Path) -> Probe {
    match platform.metadata(path) {
        Ok(m) => Ok(Some(m)),
        Err(PlatformError::Io { source, .. }) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(None)
        }
        Err(e) => Err(e.into()),
    }
}

/// Why nothing may be written below a semantic root on this machine.
#[derive(Debug, Clone, thiserror::Error)]
pub(super) enum RootUnusable {
    #[error("root {0} unavailable: {1}")]
    Unresolvable(SemanticRoot, ProbeError),
    #[error("root {0} is a symlink on this machine; never written through")]
    Symlink(SemanticRoot),
    #[error("root {0} is not a directory")]
    NotDir(SemanticRoot),
    #[error("cannot inspect root {0}: {1}")]
    Uninspectable(SemanticRoot, ProbeError),
}

/// Resolve a root and refuse it if it is a symlink or not a directory.
/// A missing root is fine (apply creates it).
pub(super) fn check_root(
    platform: &dyn Platform,
    root: SemanticRoot,
) -> Result<PathBuf, RootUnusable> {
    let abs = platform
        .resolve_root(root)
        .map_err(|e| RootUnusable::Unresolvable(root, e.into()))?;
    match probe(platform, &abs) {
        Ok(None) => Ok(abs),
        Ok(Some(m)) => match m.kind {
            FileKind::Dir => Ok(abs),
            FileKind::Symlink => Err(RootUnusable::Symlink(root)),
            FileKind::File | FileKind::Other => Err(RootUnusable::NotDir(root)),
        },
        Err(e) => Err(RootUnusable::Uninspectable(root, e)),
    }
}

/// `target` of a symlink at `link`, joined to the link's directory unless
/// absolute (not normalised). `None` if `link` has no parent.
pub(super) fn link_destination(target: &str, link: &Path) -> Option<PathBuf> {
    let t = Path::new(target);
    if t.has_root() {
        Some(t.to_path_buf())
    } else {
        Some(link.parent()?.join(t))
    }
}

/// A symlink at `link` pointing to `target` escapes: resolved lexically,
/// the target is neither inside the target root nor inside home.
pub(super) fn symlink_escapes(
    target: &str,
    link: &Path,
    root_abs: &Path,
    home: Option<&Path>,
) -> bool {
    let Some(joined) = link_destination(target, link) else {
        return false;
    };
    !normalize_abs(&joined)
        .is_some_and(|p| p.starts_with(root_abs) || home.is_some_and(|h| p.starts_with(h)))
}

#[cfg(test)]
#[path = "../../tests/plan/confine/unit.rs"]
mod tests;
