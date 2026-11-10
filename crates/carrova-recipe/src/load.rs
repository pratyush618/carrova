//! Reading recipes from disk.
//!
//! Recipe files are Carrova's own data, not user data, so they are read with
//! portable `std::fs` calls rather than through the platform adapter.

use std::fs;
use std::path::{Path, PathBuf};

use crate::{LoadError, Recipe, SUPPORTED_SCHEMA};

/// File name of a recipe inside its app directory.
pub const RECIPE_FILE: &str = "recipe.yaml";
/// Directory under the recipes root holding JSON Schemas (not a category).
pub const SCHEMA_DIR: &str = "schema";

/// A parsed recipe plus where it came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedRecipe {
    /// Path of the `recipe.yaml` file.
    pub path: PathBuf,
    /// Directory names, when loaded via [`load_tree`].
    pub layout: Option<TreeLayout>,
    pub recipe: Recipe,
}

impl LoadedRecipe {
    /// The app directory (parent of `recipe.yaml`); fixtures live in `<dir>/fixture`.
    #[must_use]
    pub fn dir(&self) -> &Path {
        self.path.parent().unwrap_or(Path::new(""))
    }
}

/// `recipes/<category_dir>/<app_dir>/recipe.yaml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeLayout {
    pub category_dir: String,
    pub app_dir: String,
}

/// Result of [`load_tree`]: every recipe that parsed, every one that did not.
#[derive(Debug, Default)]
pub struct TreeLoad {
    /// Sorted by path.
    pub recipes: Vec<LoadedRecipe>,
    pub errors: Vec<LoadError>,
}

/// Parse recipe YAML. `path` is only used in error messages.
pub fn load_str(text: &str, path: &Path) -> Result<Recipe, LoadError> {
    // Check the version first so a future-schema file gets a clear error
    // instead of an "unknown field" one.
    if let Ok(serde_yaml_ng::Value::Mapping(map)) = serde_yaml_ng::from_str(text)
        && let Some(found) = map.get("schema").and_then(serde_yaml_ng::Value::as_u64)
        && !u32::try_from(found).is_ok_and(|v| SUPPORTED_SCHEMA.contains(&v))
    {
        return Err(LoadError::UnsupportedSchema {
            path: path.to_path_buf(),
            found,
            supported: format!("{}..={}", SUPPORTED_SCHEMA.start(), SUPPORTED_SCHEMA.end()),
        });
    }
    serde_yaml_ng::from_str(text).map_err(|source| LoadError::Parse {
        path: path.to_path_buf(),
        source,
    })
}

/// Read and parse one recipe file.
pub fn load_file(path: impl AsRef<Path>) -> Result<LoadedRecipe, LoadError> {
    let path = path.as_ref();
    let text = fs::read_to_string(path).map_err(|source| LoadError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(LoadedRecipe {
        path: path.to_path_buf(),
        layout: None,
        recipe: load_str(&text, path)?,
    })
}

/// Read `<dir>/recipe.yaml`.
pub fn load_dir(dir: impl AsRef<Path>) -> Result<LoadedRecipe, LoadError> {
    let dir = dir.as_ref();
    let file = dir.join(RECIPE_FILE);
    if !file.is_file() {
        return Err(LoadError::MissingRecipe {
            dir: dir.to_path_buf(),
        });
    }
    load_file(file)
}

/// Load every `<root>/<category>/<app>/recipe.yaml`.
///
/// Skips the `schema/` directory, hidden (`.`-prefixed) directories and plain
/// files at the category/app level. Never descends below an app directory, so
/// `fixture/` trees are never read. Each app directory without a
/// `recipe.yaml` is reported as [`LoadError::MissingRecipe`]. Only failure to
/// list `root` itself is a hard error.
pub fn load_tree(root: impl AsRef<Path>) -> Result<TreeLoad, LoadError> {
    let root = root.as_ref();
    let mut out = TreeLoad::default();
    for (category_dir, cat_path) in subdirs(root)? {
        if category_dir == SCHEMA_DIR {
            continue;
        }
        let apps = match subdirs(&cat_path) {
            Ok(a) => a,
            Err(e) => {
                out.errors.push(e);
                continue;
            }
        };
        for (app_dir, app_path) in apps {
            match load_dir(&app_path) {
                Ok(mut loaded) => {
                    loaded.layout = Some(TreeLayout {
                        category_dir: category_dir.clone(),
                        app_dir,
                    });
                    out.recipes.push(loaded);
                }
                Err(e) => out.errors.push(e),
            }
        }
    }
    out.recipes.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

/// Non-hidden subdirectories of `dir` with UTF-8 names, sorted.
fn subdirs(dir: &Path) -> Result<Vec<(String, PathBuf)>, LoadError> {
    let io = |source| LoadError::Io {
        path: dir.to_path_buf(),
        source,
    };
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(io)? {
        let entry = entry.map_err(io)?;
        let Ok(name) = entry.file_name().into_string() else {
            continue;
        };
        if name.starts_with('.') || !entry.file_type().map_err(io)?.is_dir() {
            continue;
        }
        out.push((name, entry.path()));
    }
    out.sort();
    Ok(out)
}
