//! The set of recipes a scan runs against, and recipe validation.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use carrova_recipe::{
    LoadError, LoadedRecipe, RECIPE_FILE, Recipe, TreeLayout, load_dir, load_file, load_tree,
    validate_tree,
};
use serde::Serialize;
use tracing::{debug, warn};

/// Recipes that loaded and validated cleanly, plus a line per rejected file.
#[derive(Debug, Default)]
pub struct Catalog {
    /// Usable recipes, in tree order (sorted by path).
    pub recipes: Vec<Recipe>,
    /// Human-readable problems: load errors and validation issues. Recipes
    /// with any validation issue are left out of [`Catalog::recipes`].
    pub problems: Vec<String>,
}

/// The recipes root itself could not be read.
#[derive(Debug, thiserror::Error)]
#[error("cannot load recipes from {}: {source}", path.display())]
pub struct CatalogError {
    pub path: PathBuf,
    #[source]
    pub source: LoadError,
}

/// Load `<root>/<category>/<app>/recipe.yaml`, validate the whole tree and
/// keep only recipes without issues. A broken recipe never aborts the load;
/// it is reported in [`Catalog::problems`] instead.
pub fn load_catalog(root: &Path) -> Result<Catalog, CatalogError> {
    let tree = load_tree(root).map_err(|source| CatalogError {
        path: root.to_path_buf(),
        source,
    })?;
    let issues = validate_tree(&tree.recipes);
    let bad: BTreeSet<&Path> = issues.iter().filter_map(|i| i.file.as_deref()).collect();

    let mut problems: Vec<String> = tree.errors.iter().map(ToString::to_string).collect();
    problems.extend(issues.iter().map(ToString::to_string));

    let recipes: Vec<Recipe> = tree
        .recipes
        .iter()
        .filter(|l| !bad.contains(l.path.as_path()))
        .map(|l| l.recipe.clone())
        .collect();
    if !problems.is_empty() {
        warn!(
            rejected = tree.recipes.len() - recipes.len() + tree.errors.len(),
            "some recipes were skipped"
        );
    }
    debug!(
        recipes = recipes.len(),
        problems = problems.len(),
        "catalog loaded"
    );
    Ok(Catalog { recipes, problems })
}

/// Outcome of [`validate_targets`]. Each entry is one human-readable line.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize)]
pub struct ValidationReport {
    /// Recipes that parsed (after de-duplication by path) and were validated.
    pub recipes_checked: usize,
    /// Recipe files or directories that could not be read or parsed.
    pub load_errors: Vec<String>,
    /// Validation issues across all parsed recipes.
    pub issues: Vec<String>,
}

impl ValidationReport {
    /// No load errors and no validation issues.
    #[must_use]
    pub fn is_ok(&self) -> bool {
        self.load_errors.is_empty() && self.issues.is_empty()
    }
}

/// Validate recipes named by `targets`. Each target may be a `recipe.yaml`
/// file, an app directory containing one, or a recipe tree root
/// (`<root>/<category>/<app>/recipe.yaml`). Recipes reached through several
/// targets are checked once; cross-recipe checks (e.g. duplicate ids) span
/// all targets. Problems are collected, never returned as errors.
#[must_use]
pub fn validate_targets(targets: &[PathBuf]) -> ValidationReport {
    let mut loaded: Vec<LoadedRecipe> = Vec::new();
    let mut errors: Vec<LoadError> = Vec::new();
    for target in targets {
        if target.is_file() {
            push_single(load_file(target), &mut loaded, &mut errors);
        } else if target.join(RECIPE_FILE).is_file() {
            push_single(load_dir(target), &mut loaded, &mut errors);
        } else {
            match load_tree(target) {
                Ok(tree) => {
                    loaded.extend(tree.recipes);
                    errors.extend(tree.errors);
                }
                Err(e) => errors.push(e),
            }
        }
    }
    let mut seen = BTreeSet::new();
    loaded.retain(|l| seen.insert(l.path.clone()));

    let issues = validate_tree(&loaded);
    debug!(
        recipes = loaded.len(),
        issues = issues.len(),
        load_errors = errors.len(),
        "recipes validated"
    );
    ValidationReport {
        recipes_checked: loaded.len(),
        load_errors: errors.iter().map(ToString::to_string).collect(),
        issues: issues.iter().map(ToString::to_string).collect(),
    }
}

/// Single recipes loaded by path keep tree-layout checks (category/app
/// directory names) when they sit at `<category>/<app>/recipe.yaml`.
fn push_single(
    result: Result<LoadedRecipe, LoadError>,
    loaded: &mut Vec<LoadedRecipe>,
    errors: &mut Vec<LoadError>,
) {
    match result {
        Ok(mut l) => {
            if let Some(layout) = infer_layout(&l.path) {
                l.layout = Some(layout);
            }
            loaded.push(l);
        }
        Err(e) => errors.push(e),
    }
}

/// `<category>/<app>/recipe.yaml` -> directory names, when both exist.
fn infer_layout(recipe_file: &Path) -> Option<TreeLayout> {
    let name = |p: Option<&Path>| {
        p.and_then(Path::file_name)
            .and_then(|n| n.to_str())
            .map(str::to_owned)
    };
    let app_dir = recipe_file.parent();
    Some(TreeLayout {
        app_dir: name(app_dir)?,
        category_dir: name(app_dir.and_then(Path::parent))?,
    })
}

#[cfg(test)]
#[path = "../tests/catalog/unit.rs"]
mod tests;
