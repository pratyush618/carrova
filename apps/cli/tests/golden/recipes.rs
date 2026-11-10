//! `carrova recipes status` (text and JSON) and `carrova recipes list`.

use std::path::{Path, PathBuf};

use carrova_bundle::{
    BundleVersion, BundledRecipe, RecipeSource, ResolvedRecipes, SkipCause, SkippedRecipe, Tier,
};
use carrova_recipe::load_str;

use super::{golden, text};
use crate::output::print_json;
use crate::recipes::status::StatusView;
use crate::render::recipes::{recipe_list, status};

fn recipe(id: &str, category: &str, tier: Tier) -> BundledRecipe {
    let yaml = format!(
        "schema: 1\nid: {id}\nname: {id}\ncategory: {category}\n\
         detect:\n  linux:\n    - binary: {id}\n\
         items:\n  - id: config\n    label: Config\n    \
         paths: [{{ root: user_config, rel: {id} }}]\n"
    );
    BundledRecipe {
        tier,
        recipe: load_str(&yaml, Path::new("recipe.yaml")).unwrap(),
    }
}

fn resolved(source: RecipeSource, created: Option<u64>) -> ResolvedRecipes {
    ResolvedRecipes {
        source,
        recipes: vec![
            recipe("git", "dev-tools", Tier::Verified),
            recipe("ssh", "security", Tier::Verified),
            recipe("zellij", "shells", Tier::Community),
        ],
        skipped: vec![SkippedRecipe {
            id: "future-app".into(),
            reason: SkipCause::UnsupportedSchema { schema: 9 },
        }],
        plugins: Vec::new(),
        created,
        rejected_install: None,
    }
}

#[test]
fn golden_recipes_status() {
    let installed = resolved(
        RecipeSource::Installed(BundleVersion(7)),
        Some(1_800_000_000),
    );
    let shipped = resolved(
        RecipeSource::Shipped {
            dir: PathBuf::from("/usr/share/carrova/recipes"),
            version: BundleVersion(3),
        },
        Some(1_700_000_000),
    );
    let dev = resolved(
        RecipeSource::DevDir(PathBuf::from("/src/carrova/recipes")),
        None,
    );
    let view = |r: &ResolvedRecipes| StatusView::from(r.in_use());
    let mut s = text(|o| print_json(o, &view(&installed)).map_err(std::io::Error::other));
    for r in [&installed, &shipped, &dev] {
        s.push_str("--\n");
        s.push_str(&text(|o| status(o, &view(r))));
    }
    s.push_str("--\n");
    s.push_str(&text(|o| {
        print_json(o, &view(&dev)).map_err(std::io::Error::other)
    }));
    golden("recipes_status", &s);
}

/// A signed bundle shows each recipe's tier; an unsigned dev tree shows
/// every recipe as `unverified`, whatever its `TIERS.yaml` entry says.
#[test]
fn golden_recipes_list() {
    let installed = resolved(RecipeSource::Installed(BundleVersion(7)), None);
    let dev = resolved(
        RecipeSource::DevDir(PathBuf::from("/src/carrova/recipes")),
        None,
    );
    let mut s = text(|o| recipe_list(o, &installed));
    s.push_str("--\n");
    s.push_str(&text(|o| recipe_list(o, &dev)));
    golden("recipes_list", &s);
}
