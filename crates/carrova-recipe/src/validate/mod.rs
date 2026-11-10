//! Semantic validation beyond what serde enforces.
use std::collections::HashMap;
use std::path::Path;

use crate::{LoadedRecipe, Recipe};

mod identity;
mod issue;
mod items;
mod os_paths;
mod sensitive;

use identity::{check_detect, check_identity, check_processes};
pub use issue::{Issue, IssueKind};
use items::check_items;
use sensitive::check_sensitive_capture;

/// First recipe schema version of each versioned feature (one source of
/// truth for the validator and its messages).
mod since {
    /// Recipe-level `processes`.
    pub(super) const PROCESSES: u32 = 2;
    /// Item `handler` naming a built-in.
    pub(super) const HANDLER: u32 = 2;
    /// Item `handler: { plugin: <id> }`.
    pub(super) const PLUGIN_HANDLER: u32 = 3;
    /// Item per-OS `os` overrides.
    pub(super) const OS_OVERRIDES: u32 = 3;
    /// Detection rules for any OS instead of required Linux rules.
    pub(super) const ANY_OS_DETECT: u32 = 3;
}

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
    /// Record an issue; an identical one is recorded once (checks that run
    /// once per OS report a path shared by several OSes only once).
    fn push(&mut self, location: impl Into<String>, kind: IssueKind) {
        let issue = Issue {
            file: self.file.map(Path::to_path_buf),
            recipe: self.recipe.to_owned(),
            location: location.into(),
            kind,
        };
        if !self.issues.contains(&issue) {
            self.issues.push(issue);
        }
    }

    /// Report `field` (at `location`) if `schema` predates `min`.
    fn require_schema(
        &mut self,
        schema: u32,
        location: impl Into<String>,
        field: &'static str,
        min: u32,
    ) {
        if schema < min {
            self.push(location, IssueKind::RequiresSchema { field, min });
        }
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
