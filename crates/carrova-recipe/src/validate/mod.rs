//! Semantic validation beyond what serde enforces.
use std::collections::HashMap;
use std::path::Path;

use crate::{LoadedRecipe, Recipe};

mod identity;
mod issue;
mod items;

use identity::{check_detect, check_identity, check_processes};
pub use issue::{Issue, IssueKind};
use items::{check_items, check_sensitive_capture};

/// `^[a-z0-9]+(-[a-z0-9]+)*$`
#[must_use]
pub fn is_kebab(s: &str) -> bool {
    !s.is_empty()
        && s.split('-').all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}

/// Not empty, no path separators or NUL (binary and process names).
fn is_bare_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(['/', '\\', '\0'])
}

struct Sink<'a> {
    file: Option<&'a Path>,
    recipe: &'a str,
    issues: Vec<Issue>,
}

impl Sink<'_> {
    fn push(&mut self, location: impl Into<String>, kind: IssueKind) {
        self.issues.push(Issue {
            file: self.file.map(Path::to_path_buf),
            recipe: self.recipe.to_owned(),
            location: location.into(),
            kind,
        });
    }
}

/// Check one recipe on its own. Returns every problem found.
#[must_use]
pub fn validate(recipe: &Recipe) -> Vec<Issue> {
    validate_with(recipe, None, None)
}

/// [`validate`] plus directory-name checks when the recipe came from
/// [`crate::load_tree`]. Issues carry the file path.
#[must_use]
pub fn validate_loaded(loaded: &LoadedRecipe) -> Vec<Issue> {
    validate_with(&loaded.recipe, Some(&loaded.path), loaded.layout.as_ref())
}

/// [`validate_loaded`] for every recipe plus cross-recipe checks (unique ids).
#[must_use]
pub fn validate_tree(recipes: &[LoadedRecipe]) -> Vec<Issue> {
    let mut issues: Vec<Issue> = recipes.iter().flat_map(validate_loaded).collect();
    let mut seen: HashMap<&str, &Path> = HashMap::new();
    for loaded in recipes {
        let id = loaded.recipe.id.as_str();
        if let Some(first) = seen.get(id) {
            issues.push(Issue {
                file: Some(loaded.path.clone()),
                recipe: id.to_owned(),
                location: "id".into(),
                kind: IssueKind::DuplicateRecipeId {
                    id: id.to_owned(),
                    other: first.to_path_buf(),
                },
            });
        } else {
            seen.insert(id, &loaded.path);
        }
    }
    issues
}

fn validate_with(
    r: &Recipe,
    file: Option<&Path>,
    layout: Option<&crate::TreeLayout>,
) -> Vec<Issue> {
    let mut s = Sink {
        file,
        recipe: &r.id,
        issues: Vec::new(),
    };
    check_identity(r, layout, &mut s);
    check_detect(r, &mut s);
    check_processes(r, &mut s);
    check_items(r, &mut s);
    check_sensitive_capture(r, &mut s);
    s.issues
}
