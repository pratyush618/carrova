//! Carrova recipes: load, validate, detect apps, resolve items.
//!
//! A recipe is declarative data (`recipes/<category>/<app>/recipe.yaml`) that
//! says how to detect an app and which items (files/dirs) can move. The JSON
//! Schemas at `recipes/schema/recipe.v<N>.json` are authoritative for the
//! shape; [`validate`] adds the semantic checks a schema cannot express.
//!
//! Schema v2 adds item `handler` (a built-in [`Handler`]) and recipe-level
//! `processes`; a v1 recipe using either is a validation error. Schema v3
//! adds per-OS item paths ([`OsOverrides`], read via [`Item::paths_for`]),
//! plugin handlers ([`HandlerRef::Plugin`]) and recipes for any OS (detect
//! rules for at least one OS instead of Linux).
//!
//! Typical flow:
//!
//! 1. [`load_tree`] (or [`load_dir`] / [`load_file`]) parses recipes.
//! 2. [`validate_tree`] / [`validate_loaded`] / [`validate`] report every problem.
//! 3. [`detect`] checks whether an app is present via a [`Platform`].
//! 4. [`resolve_item`] lists the files an item would move.
//!
//! All OS access for detection and metadata goes through
//! [`carrova_platform::Platform`]; directory walking uses portable `std::fs`
//! APIs (via `walkdir`) and never follows symlinks.
//!
//! [`Platform`]: carrova_platform::Platform

mod detect;
mod error;
mod handler;
mod load;
mod model;
mod os_paths;
mod plugin_id;
mod relpath;
mod resolve;
mod validate;

use std::ops::RangeInclusive;

pub use carrova_platform::SemanticRoot;
pub use detect::{Detection, Evidence, detect};
pub use error::LoadError;
pub use handler::{Handler, HandlerRef};
pub use load::{
    LoadedRecipe, RECIPE_FILE, SCHEMA_DIR, TreeLayout, TreeLoad, load_dir, load_file, load_str,
    load_tree,
};
pub use model::{Detect, DetectRule, Flags, Item, PathRef, Portability, Recipe};
pub use os_paths::{OsOverrides, OsPaths};
pub use plugin_id::{PluginId, PluginIdError};
pub use relpath::{RelPath, RelPathError};
pub use resolve::{FileEntry, ResolvedItem, SkipReason, Skipped, build_excludes, resolve_item};
pub use validate::{Issue, IssueKind, is_kebab, validate, validate_loaded, validate_tree};

/// Recipe schema versions this engine understands.
pub const SUPPORTED_SCHEMA: RangeInclusive<u32> = 1..=3;
