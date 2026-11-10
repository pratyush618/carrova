//! Semantic roots and the state dir: XDG base dirs, `user-dirs.dirs`, and
//! confinement (a root is never the filesystem root).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use super::LinuxPlatform;
use crate::sys::confine;
use crate::{PlatformError, SemanticRoot};

impl LinuxPlatform {
    /// Path of `root` per the XDG base-dir and user-dirs rules.
    pub(super) fn root_path(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        match root {
            SemanticRoot::Home => self.home(),
            SemanticRoot::UserConfig => {
                self.xdg_dir(root, self.env.xdg_config_home.as_ref(), ".config")
            }
            SemanticRoot::UserData => {
                self.xdg_dir(root, self.env.xdg_data_home.as_ref(), ".local/share")
            }
            SemanticRoot::Cache => self.xdg_dir(root, self.env.xdg_cache_home.as_ref(), ".cache"),
            SemanticRoot::Documents => self.documents(),
        }
    }

    pub(super) fn home(&self) -> Result<PathBuf, PlatformError> {
        crate::sys::home_dir(self.env.home.as_deref())
    }

    /// XDG base-dir rule: use the variable only if set, non-empty and absolute.
    fn xdg_dir(
        &self,
        root: SemanticRoot,
        var: Option<&OsString>,
        default_rel: &str,
    ) -> Result<PathBuf, PlatformError> {
        if let Some(v) = var {
            let p = Path::new(v);
            if p.is_absolute() {
                return confine(root, p);
            }
        }
        confine(root, &self.home()?.join(default_rel))
    }

    /// `$XDG_STATE_HOME/carrova` if the variable is set and absolute, else
    /// `$HOME/.local/state/carrova`; lexically normalised. The base directory
    /// may not be (or climb above) the filesystem root.
    pub(super) fn state_dir_impl(&self) -> Result<PathBuf, PlatformError> {
        let base = match self.env.xdg_state_home.as_ref().map(Path::new) {
            Some(v) if v.is_absolute() => v.to_path_buf(),
            _ => self.home()?.join(".local/state"),
        };
        let base = crate::path::normalize_abs(&base)
            .filter(|b| b.parent().is_some())
            .ok_or_else(|| {
                PlatformError::StateDirUnavailable(
                    "XDG_STATE_HOME is, or climbs above, the filesystem root".into(),
                )
            })?;
        Ok(base.join("carrova"))
    }

    fn documents(&self) -> Result<PathBuf, PlatformError> {
        let config = self.xdg_dir(
            SemanticRoot::UserConfig,
            self.env.xdg_config_home.as_ref(),
            ".config",
        )?;
        // Best effort: a missing/unreadable/non-UTF-8 user-dirs.dirs means "use default".
        if let Ok(text) = std::fs::read_to_string(config.join("user-dirs.dirs"))
            && let Some(value) = parse_user_dir(&text, "XDG_DOCUMENTS_DIR")
            && let Some(p) = expand_user_dir(&value, || self.home())?
        {
            return confine(SemanticRoot::Documents, &p);
        }
        confine(SemanticRoot::Documents, &self.home()?.join("Documents"))
    }
}

/// Find the last `KEY="value"` assignment in a `user-dirs.dirs` file and
/// return the unquoted, unescaped value.
fn parse_user_dir(text: &str, key: &str) -> Option<String> {
    let mut found = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let Some(rest) = line.strip_prefix(key) else {
            continue;
        };
        let Some(raw) = rest.trim_start().strip_prefix('=') else {
            continue;
        };
        found = Some(unquote(raw.trim()));
    }
    found
}

/// Shell-ish unquoting: `"..."` with `\"`/`\\`/`\$` escapes, `'...'` literal,
/// or a bare word (ends at whitespace / `#`).
fn unquote(raw: &str) -> String {
    let mut out = String::new();
    let mut chars = raw.chars();
    match raw.chars().next() {
        Some('"') => {
            chars.next();
            while let Some(c) = chars.next() {
                match c {
                    '"' => break,
                    '\\' => {
                        if let Some(n) = chars.next() {
                            out.push(n);
                        }
                    }
                    c => out.push(c),
                }
            }
        }
        Some('\'') => {
            chars.next();
            out.extend(chars.take_while(|&c| c != '\''));
        }
        _ => out.extend(chars.take_while(|&c| !c.is_whitespace() && c != '#')),
    }
    out
}

/// Expand a `user-dirs.dirs` value. Valid forms are `$HOME/rel`, `${HOME}/rel`,
/// `$HOME` and absolute paths; anything else yields `None` (caller falls back).
fn expand_user_dir(
    value: &str,
    home: impl Fn() -> Result<PathBuf, PlatformError>,
) -> Result<Option<PathBuf>, PlatformError> {
    for prefix in ["$HOME", "${HOME}"] {
        if let Some(rest) = value.strip_prefix(prefix) {
            if rest.is_empty() {
                return Ok(Some(home()?));
            }
            if let Some(rel) = rest.strip_prefix('/') {
                let rel = rel.trim_start_matches('/');
                return Ok(Some(home()?.join(rel)));
            }
        }
    }
    let p = Path::new(value);
    Ok(p.is_absolute().then(|| p.to_path_buf()))
}
