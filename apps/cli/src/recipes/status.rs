//! `carrova recipes status`: which recipes are in use, as a view model
//! shared by the text and JSON output. Built from the recipes service's
//! [`RecipesInUse`] (the summary the desktop app shows too).

use std::path::PathBuf;

use carrova_bundle::{RecipeSource, RecipesInUse, TierCounts};
use serde::Serialize;

/// Everything `carrova recipes status` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct StatusView {
    /// Where the recipes come from.
    pub source: SourceView,
    /// Whether the recipes are signature-verified.
    pub verified: bool,
    /// When the bundle was built (unix seconds); `None` for a dev dir.
    pub created: Option<u64>,
    /// Usable recipes by effective tier (all unverified for a dev dir).
    pub recipes: TierCounts,
    /// Recipes left out, with the reason.
    pub skipped: Vec<SkippedView>,
}

/// [`RecipeSource`] in output form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SourceView {
    /// An unsigned recipe tree (`--recipes` or a debug build's checkout).
    DevDir {
        /// The tree.
        path: PathBuf,
    },
    /// The bundle installed by `carrova recipes update`.
    Installed {
        /// Bundle version.
        version: u64,
    },
    /// The bundle shipped with this install.
    Shipped {
        /// Bundle version.
        version: u64,
        /// Bundle directory.
        path: PathBuf,
    },
}

/// A recipe left out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SkippedView {
    /// Recipe id.
    pub id: String,
    /// Why it was left out.
    pub reason: String,
}

impl From<RecipeSource> for SourceView {
    fn from(source: RecipeSource) -> Self {
        match source {
            RecipeSource::DevDir(path) => Self::DevDir { path },
            RecipeSource::Installed(v) => Self::Installed { version: v.0 },
            RecipeSource::Shipped { dir, version } => Self::Shipped {
                version: version.0,
                path: dir,
            },
        }
    }
}

impl From<RecipesInUse> for StatusView {
    fn from(in_use: RecipesInUse) -> Self {
        Self {
            source: in_use.source.into(),
            verified: in_use.verified,
            created: in_use.created,
            recipes: in_use.counts,
            skipped: in_use
                .skipped
                .into_iter()
                .map(|s| SkippedView {
                    reason: s.reason.to_string(),
                    id: s.id,
                })
                .collect(),
        }
    }
}
