//! Semantic roots and the state dir: fixed folders under `$HOME/Library`
//! (macOS has no XDG-style overrides), confined like every root.

use std::path::PathBuf;

use super::MacPlatform;
use crate::sys::confine;
use crate::{PlatformError, SemanticRoot};

/// Per-user config and data both live here on macOS.
const APP_SUPPORT: &str = "Library/Application Support";
/// Per-user caches.
const CACHES: &str = "Library/Caches";

impl MacPlatform {
    /// Path of `root`: `~/Library/Application Support` (config, data),
    /// `~/Library/Caches`, `~/Documents` or `$HOME`.
    pub(super) fn root_path(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        let rel = match root {
            SemanticRoot::Home => return self.home(),
            SemanticRoot::UserConfig | SemanticRoot::UserData => APP_SUPPORT,
            SemanticRoot::Cache => CACHES,
            SemanticRoot::Documents => "Documents",
        };
        confine(root, &self.home()?.join(rel))
    }

    pub(super) fn home(&self) -> Result<PathBuf, PlatformError> {
        crate::sys::home_dir(self.env.home.as_deref())
    }

    /// `~/Library/Application Support/Carrova/state`.
    pub(super) fn state_dir_impl(&self) -> Result<PathBuf, PlatformError> {
        Ok(self.home()?.join(APP_SUPPORT).join("Carrova/state"))
    }
}
