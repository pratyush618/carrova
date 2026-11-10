//! `carrova recipes …`: validate and list recipes, show which recipes are in
//! use, update, roll back or reset the installed bundle. Also where every
//! other command gets its recipes from ([`RecipeLocations`] +
//! [`LoadRecipes`]).

mod error;
pub mod seams;
pub mod source;
pub mod status;
mod update;

#[cfg(test)]
#[path = "../../tests/recipes/unit.rs"]
mod tests;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Result, bail};
use carrova_core::validate_targets;
use clap::Subcommand;

pub use carrova_bundle::RecipeLocations;
pub use source::{LoadRecipes, for_this_exe};

use crate::output::{OutputFormat, print_json};
use crate::render;
use update::{ResetResult, RollbackResult};

/// `carrova recipes` subcommands.
#[derive(Debug, Subcommand)]
pub enum RecipesCommand {
    /// Validate recipes; exits non-zero on any problem.
    Validate {
        /// Recipe directories, recipe.yaml files or recipe tree roots
        /// (default: the --recipes directory; in a development build, the
        /// source checkout's recipes).
        paths: Vec<PathBuf>,
    },
    /// List available recipes.
    List,
    /// Show which recipes are in use: source, version, counts by tier.
    Status {
        /// Print the status as JSON on stdout.
        #[arg(long)]
        json: bool,
    },
    /// Download and install the latest verified recipe bundle.
    ///
    /// Recipes are only ever updated by this command, never automatically.
    Update {
        /// Release download base URL; its `recipes-latest/latest.json`
        /// names the `recipes-vN` release to install.
        #[arg(long, value_name = "URL", default_value = carrova_bundle::DEFAULT_BASE_URL)]
        url: String,
        /// Only check whether a newer bundle is available; install nothing.
        #[arg(long)]
        check: bool,
    },
    /// Go back to the recipe bundle the last update replaced.
    Rollback {
        /// Don't ask for confirmation (required when stdin is not a terminal).
        #[arg(short, long)]
        yes: bool,
    },
    /// Remove the installed recipe bundle and use the recipes shipped with
    /// Carrova again (e.g. after the installed bundle was revoked).
    Reset {
        /// Don't ask for confirmation (required when stdin is not a terminal).
        #[arg(short, long)]
        yes: bool,
    },
}

/// Run a `carrova recipes` subcommand.
pub fn run(
    out: &mut impl Write,
    recipes: &RecipeLocations,
    command: RecipesCommand,
) -> Result<ExitCode> {
    match command {
        RecipesCommand::Validate { paths } => validate(out, recipes, paths),
        RecipesCommand::List => {
            let resolved = recipes.load()?;
            source::warn_skipped(&resolved.skipped);
            render::recipes::recipe_list(out, &resolved)?;
            Ok(ExitCode::SUCCESS)
        }
        RecipesCommand::Status { json } => {
            let view = status::StatusView::from(recipes.load()?.in_use());
            match OutputFormat::from_json_flag(json) {
                OutputFormat::Json => print_json(out, &view)?,
                OutputFormat::Human => render::recipes::status(out, &view)?,
            }
            Ok(ExitCode::SUCCESS)
        }
        RecipesCommand::Update { url, check: true } => {
            render::recipes::check_outcome(out, &update::check(&url)?)?;
            Ok(ExitCode::SUCCESS)
        }
        RecipesCommand::Update { url, check: false } => {
            render::recipes::update_outcome(out, &update::update(&url)?)?;
            Ok(ExitCode::SUCCESS)
        }
        RecipesCommand::Rollback { yes } => match update::rollback(yes)? {
            RollbackResult::RolledBack { from, to } => {
                writeln!(
                    out,
                    "Recipes rolled back from {from} to {to}. \
                     `carrova recipes update` installs the latest bundle again."
                )?;
                Ok(ExitCode::SUCCESS)
            }
            RollbackResult::Declined => {
                eprintln!("rollback cancelled");
                Ok(ExitCode::FAILURE)
            }
        },
        RecipesCommand::Reset { yes } => match update::reset(yes)? {
            ResetResult::Reset { removed } => {
                render::recipes::reset_outcome(out, removed)?;
                Ok(ExitCode::SUCCESS)
            }
            ResetResult::Declined => {
                eprintln!("reset cancelled");
                Ok(ExitCode::FAILURE)
            }
        },
    }
}

/// `carrova recipes validate [PATH...]`: recipe trees, not bundles (a
/// bundle is validated when it is built and verified when it is loaded).
fn validate(
    out: &mut impl Write,
    recipes: &RecipeLocations,
    paths: Vec<PathBuf>,
) -> Result<ExitCode> {
    let targets = if paths.is_empty() {
        let Some(tree) = recipes.dev_tree() else {
            bail!("nothing to validate: pass recipe paths or --recipes DIR");
        };
        vec![tree.to_path_buf()]
    } else {
        paths
    };
    let report = validate_targets(&targets);
    render::scan::validation_report(out, &report)?;
    Ok(if report.is_ok() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}
