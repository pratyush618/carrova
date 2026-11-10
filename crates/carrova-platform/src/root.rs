use std::fmt;

use serde::{Deserialize, Serialize};

/// OS-neutral anchor for every path Carrova touches.
///
/// Recipes and manifests only ever store `(SemanticRoot, relative path)`;
/// the platform adapter maps a root to a concrete directory.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum SemanticRoot {
    /// Per-user configuration (Linux: `$XDG_CONFIG_HOME`, default `~/.config`).
    UserConfig,
    /// Per-user application data (Linux: `$XDG_DATA_HOME`, default `~/.local/share`).
    UserData,
    /// Per-user cache (Linux: `$XDG_CACHE_HOME`, default `~/.cache`).
    Cache,
    /// The user's home directory.
    Home,
    /// The user's documents directory (Linux: `XDG_DOCUMENTS_DIR`, default `~/Documents`).
    Documents,
}

impl SemanticRoot {
    /// All roots, in stable order.
    pub const ALL: [Self; 5] = [
        Self::UserConfig,
        Self::UserData,
        Self::Cache,
        Self::Home,
        Self::Documents,
    ];

    /// Stable snake_case name, identical to the serde representation.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UserConfig => "user_config",
            Self::UserData => "user_data",
            Self::Cache => "cache",
            Self::Home => "home",
            Self::Documents => "documents",
        }
    }
}

impl fmt::Display for SemanticRoot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
