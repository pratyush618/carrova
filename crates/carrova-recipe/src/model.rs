//! Serde types mirroring `recipes/schema/recipe.v{1,2}.json`.

use std::fmt;

use carrova_platform::OsKind;
use serde::{Deserialize, Serialize};

use crate::{RelPath, SemanticRoot};

/// One app recipe (schema v1 or v2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Recipe {
    /// Schema version; must be in [`crate::SUPPORTED_SCHEMA`].
    pub schema: u32,
    /// App id, lowercase kebab-case, unique across recipes.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Category, lowercase kebab-case (= parent directory name).
    pub category: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub detect: Detect,
    /// Schema v2: extra process names (daemons, helpers) that count as "app
    /// running" for `requires_closed` items, besides the detect binaries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub processes: Vec<String>,
    pub items: Vec<Item>,
}

impl Recipe {
    /// Process names that mean "app running" on `os`: every `binary` detect
    /// rule for that OS, then every recipe-level `processes` entry.
    pub fn process_names(&self, os: OsKind) -> impl Iterator<Item = &str> {
        self.detect
            .rules(os)
            .iter()
            .filter_map(|r| match r {
                DetectRule::Binary(b) => Some(b.as_str()),
                DetectRule::Path(_) => None,
            })
            .chain(self.processes.iter().map(String::as_str))
    }
}

/// Detection rules per OS family. Any one matching rule = detected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Detect {
    pub linux: Vec<DetectRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub windows: Vec<DetectRule>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub macos: Vec<DetectRule>,
}

impl Detect {
    /// Rules for one OS family.
    #[must_use]
    pub fn rules(&self, os: OsKind) -> &[DetectRule] {
        match os {
            OsKind::Linux => &self.linux,
            OsKind::MacOs => &self.macos,
            OsKind::Windows => &self.windows,
        }
    }
}

/// A single detection rule. YAML: `- binary: git` or
/// `- path: { root: home, rel: .gitconfig }`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "RawDetectRule", into = "RawDetectRule")]
pub enum DetectRule {
    /// Bare executable name, looked up on the user's search path.
    Binary(String),
    /// A path that exists (any kind; symlinks not followed).
    Path(PathRef),
}

/// Wire form of [`DetectRule`]: a map with exactly one of the two keys.
/// (serde_yaml_ng cannot read externally tagged enums from plain maps.)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawDetectRule {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    binary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    path: Option<PathRef>,
}

impl TryFrom<RawDetectRule> for DetectRule {
    type Error = &'static str;
    fn try_from(raw: RawDetectRule) -> Result<Self, Self::Error> {
        match (raw.binary, raw.path) {
            (Some(b), None) => Ok(Self::Binary(b)),
            (None, Some(p)) => Ok(Self::Path(p)),
            (None, None) => Err("detect rule needs exactly one of `binary` or `path`, got none"),
            (Some(_), Some(_)) => {
                Err("detect rule needs exactly one of `binary` or `path`, got both")
            }
        }
    }
}

impl From<DetectRule> for RawDetectRule {
    fn from(rule: DetectRule) -> Self {
        match rule {
            DetectRule::Binary(b) => Self {
                binary: Some(b),
                path: None,
            },
            DetectRule::Path(p) => Self {
                binary: None,
                path: Some(p),
            },
        }
    }
}

/// `(semantic root, relative path)` — the only way recipes name paths.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PathRef {
    pub root: SemanticRoot,
    pub rel: RelPath,
}

impl std::fmt::Display for PathRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.root, self.rel)
    }
}

/// A selectable unit of an app's data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// Lowercase kebab-case, unique within the recipe.
    pub id: String,
    pub label: String,
    /// Required (non-empty) unless [`Item::unmovable`] is set.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub paths: Vec<PathRef>,
    /// Exclude globs; see [`crate::build_excludes`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exclude: Vec<String>,
    #[serde(default)]
    pub flags: Flags,
    /// Reason this item cannot move. Shown greyed out, never captured.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unmovable: Option<String>,
    /// Schema v2: capture/apply through this built-in handler instead of
    /// copying raw files. [`Item::paths`] then declare the files the handler
    /// mutates on the receiver (plan + rollback backup).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub handler: Option<Handler>,
}

/// Built-in Rust handlers an item may name (closed set; recipes stay data).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Handler {
    /// GnuPG public keys + ownertrust via `gpg --export` / `--import`.
    GpgKeyring,
}

impl Handler {
    /// Every handler, in declaration order.
    pub const ALL: &'static [Self] = &[Self::GpgKeyring];

    /// Stable wire name (same as the YAML spelling), e.g. `gpg-keyring`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpgKeyring => "gpg-keyring",
        }
    }

    /// Inverse of [`Handler::as_str`].
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|h| h.as_str() == name)
    }
}

impl fmt::Display for Handler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Item {
    /// Whether the item is pre-selected in the picker.
    ///
    /// `flags.default` if given, else `true` — but sensitive and unmovable
    /// items are always off by default.
    #[must_use]
    pub fn default_selected(&self) -> bool {
        self.unmovable.is_none() && !self.flags.sensitive && self.flags.default.unwrap_or(true)
    }

    /// Whether the item may ever be captured.
    #[must_use]
    pub fn is_movable(&self) -> bool {
        self.unmovable.is_none()
    }
}

/// Item flags. Every field is optional in YAML.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Flags {
    /// Default `same_os`.
    #[serde(default)]
    pub portability: Portability,
    /// Secret material; needs explicit confirmation. Default `false`.
    #[serde(default)]
    pub sensitive: bool,
    /// App must be closed while capturing/applying. Default `false`.
    #[serde(default)]
    pub requires_closed: bool,
    /// Explicit pre-selection. `None` = use [`Item::default_selected`] rules.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<bool>,
}

/// Where an item can be restored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Portability {
    /// Only onto the same OS family (v1: everything).
    #[default]
    SameOs,
    /// OS-neutral content.
    CrossOs,
}
