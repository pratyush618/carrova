//! Schema v3 per-OS path overrides of an item ([`OsOverrides`]).

use carrova_platform::OsKind;
use serde::{Deserialize, Deserializer, Serialize};

use crate::{PathRef, RelPath, RelPathError, SemanticRoot};

/// Per-OS replacements for an item's `paths` / `exclude` (schema 3+).
///
/// An OS without an entry uses the item's base paths. Override paths are
/// *slot-aligned* with the base: `os.<x>.paths[i]` is where the data of
/// base `paths[i]` lives on `<x>`, which is how a cross-OS restore maps a
/// source path to a local one (validation enforces equal lengths).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsOverrides {
    /// Paths on Linux.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<OsPaths>,
    /// Paths on macOS.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub macos: Option<OsPaths>,
    /// Paths on Windows. Its `rel` values may also use `\\` separators
    /// (normalised to `/`), see [`RelPath::new_windows`].
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "windows_paths"
    )]
    pub windows: Option<OsPaths>,
}

impl OsOverrides {
    /// The override for `os`, if any.
    #[must_use]
    pub const fn get(&self, os: OsKind) -> Option<&OsPaths> {
        match os {
            OsKind::Linux => self.linux.as_ref(),
            OsKind::MacOs => self.macos.as_ref(),
            OsKind::Windows => self.windows.as_ref(),
        }
    }

    /// Every present override with its OS, in [`OsKind::ALL`] order.
    pub fn iter(&self) -> impl Iterator<Item = (OsKind, &OsPaths)> {
        OsKind::ALL
            .into_iter()
            .filter_map(|os| self.get(os).map(|p| (os, p)))
    }
}

/// An item's paths on one OS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OsPaths {
    /// Replaces the base `paths`, slot by slot (same length).
    pub paths: Vec<PathRef>,
    /// Replaces the base `exclude` when present (`[]` = no excludes on this
    /// OS); absent = inherit the base globs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exclude: Option<Vec<String>>,
}

/// A [`PathRef`] read from a Windows-only block: `rel` is parsed with
/// [`RelPath::new_windows`] (`\\` is only a separator on Windows).
#[derive(Deserialize)]
#[serde(try_from = "RawPathRef")]
pub(crate) struct WindowsPathRef(pub(crate) PathRef);

/// Wire form of [`WindowsPathRef`].
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawPathRef {
    root: SemanticRoot,
    rel: String,
}

impl TryFrom<RawPathRef> for WindowsPathRef {
    type Error = RelPathError;
    fn try_from(raw: RawPathRef) -> Result<Self, Self::Error> {
        let rel = RelPath::new_windows(&raw.rel)?;
        Ok(Self(PathRef {
            root: raw.root,
            rel,
        }))
    }
}

/// Deserialize an `os.windows` block, its paths as [`WindowsPathRef`]s.
fn windows_paths<'de, D: Deserializer<'de>>(d: D) -> Result<Option<OsPaths>, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct RawOsPaths {
        paths: Vec<WindowsPathRef>,
        #[serde(default)]
        exclude: Option<Vec<String>>,
    }

    Ok(Option::<RawOsPaths>::deserialize(d)?.map(|raw| OsPaths {
        paths: raw.paths.into_iter().map(|p| p.0).collect(),
        exclude: raw.exclude,
    }))
}
