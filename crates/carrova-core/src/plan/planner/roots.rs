//! The receiver's semantic roots as the planner sees them: usable for
//! writing or not, and their resolved paths for matching.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use carrova_platform::path::normalize_abs;
use carrova_platform::{Platform, SemanticRoot};

use crate::key::RootedPath;
use crate::plan::DANGEROUS_PATHS;
use crate::plan::DangerousPath;
use crate::plan::confine::{RootUnusable, check_root};

/// Every root, inspected once per plan.
pub(super) struct Roots {
    /// Where each root may be written ([`check_root`]).
    usable: HashMap<SemanticRoot, Result<PathBuf, RootUnusable>>,
    /// Lexically normalised root paths (even unusable ones), for matching.
    resolved: HashMap<SemanticRoot, PathBuf>,
    /// The home root, if usable.
    home: Option<PathBuf>,
}

impl Roots {
    pub(super) fn inspect(platform: &dyn Platform) -> Self {
        let mut usable = HashMap::new();
        let mut resolved = HashMap::new();
        for root in SemanticRoot::ALL {
            usable.insert(root, check_root(platform, root));
            if let Some(abs) = platform
                .resolve_root(root)
                .ok()
                .and_then(|p| normalize_abs(&p))
            {
                resolved.insert(root, abs);
            }
        }
        let home = usable
            .get(&SemanticRoot::Home)
            .and_then(|r| r.as_ref().ok())
            .cloned();
        Self {
            usable,
            resolved,
            home,
        }
    }

    /// The root's directory, or why nothing may be written below it.
    pub(super) fn usable(&self, root: SemanticRoot) -> Option<&Result<PathBuf, RootUnusable>> {
        self.usable.get(&root)
    }

    /// The home root, if usable.
    pub(super) fn home(&self) -> Option<&Path> {
        self.home.as_deref()
    }

    /// Is `path` equal to or below `base` — by name or by resolved
    /// absolute path?
    pub(super) fn within(&self, path: RootedPath<'_>, base: RootedPath<'_>) -> bool {
        if path.is_within(base) {
            return true;
        }
        match (self.resolved.get(&path.root), self.resolved.get(&base.root)) {
            (Some(a), Some(b)) => {
                let mut a = a.clone();
                a.extend(path.rel.split('/'));
                let mut b = b.clone();
                b.extend(base.rel.split('/'));
                a.starts_with(&b)
            }
            _ => false,
        }
    }

    /// First [`DANGEROUS_PATHS`] entry covering `(root, rel)`.
    pub(super) fn dangerous(
        &self,
        root: SemanticRoot,
        rel: &str,
    ) -> Option<&'static DangerousPath> {
        let path = RootedPath::new(root, rel);
        DANGEROUS_PATHS.iter().find(|d| {
            self.within(path, RootedPath::new(d.root, d.rel))
                || d.home_alias()
                    .is_some_and(|a| self.within(path, RootedPath::new(SemanticRoot::Home, &a)))
        })
    }
}
