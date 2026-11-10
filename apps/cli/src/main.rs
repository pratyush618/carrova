//! `carrova` command-line front-end. Parses arguments, calls the engine and
//! renders its results; no business logic lives here.

mod args;
mod events;
mod interrupt;
mod lan;
mod output;
mod prompt;
mod recipes;
mod render;
mod report;
mod session;

#[cfg(test)]
#[path = "../tests/golden/unit.rs"]
mod tests;

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::{Result, anyhow};
use carrova_core::{ScanOptions, scan};
use carrova_platform::PlatformError;
use clap::{Parser, Subcommand};

use crate::output::{OutputFormat, print_json};
use crate::recipes::{LoadRecipes, RecipeLocations, RecipesCommand};
use crate::report::Reports;

/// Move app data (configs, keys, user data) between computers.
#[derive(Debug, Parser)]
#[command(name = "carrova", version)]
struct Cli {
    /// Unverified recipe tree to use instead of the verified recipe bundle
    /// (for recipe development; a warning is printed). Default:
    /// $CARROVA_RECIPES, else the installed or shipped bundle.
    #[arg(long, global = true, value_name = "DIR")]
    recipes: Option<PathBuf>,

    /// More logging on stderr (-v info, -vv debug, -vvv trace). RUST_LOG overrides.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Work with recipes.
    Recipes {
        #[command(subcommand)]
        command: RecipesCommand,
    },
    /// Show detected apps, their items and sizes, and unrecognised data.
    Scan {
        /// Print the full scan report as JSON on stdout.
        #[arg(long)]
        json: bool,
        /// Also list apps that were not detected.
        #[arg(long)]
        all: bool,
    },
    /// Export selected app data to an encrypted archive file.
    ///
    /// The passphrase is read from CARROVA_PASSPHRASE if set (automation,
    /// tests), else prompted twice on the terminal.
    Export(session::ExportArgs),
    /// Import an archive: show the plan, then apply it after confirmation.
    ///
    /// The passphrase is read from CARROVA_PASSPHRASE if set (automation,
    /// tests), else prompted on the terminal.
    Import(session::ImportArgs),
    /// Send selected app data to another machine on this network.
    ///
    /// Shows a pairing code to type on the other machine (`carrova
    /// receive`). The other machine reviews and approves its own plan; this
    /// one shows what it found and decided.
    Send(lan::SendArgs),
    /// Receive app data from `carrova send` on another machine: pair with
    /// its code, then review the plan and apply it after confirmation.
    Receive(lan::ReceiveArgs),
    /// Undo an applied import from its rollback point.
    Rollback(session::RollbackArgs),
    /// List, inspect and dismiss sessions.
    Sessions(session::SessionsArgs),
    /// Opt-in local error reports: choose, list, inspect, delete, or turn
    /// one into a GitHub issue you file yourself.
    Report(report::ReportArgs),
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    interrupt::install();
    let reports = Reports::from_platform(carrova_platform::current().into());
    if let Ok(reports) = &reports {
        reports.install_panic_hook();
    }
    // `carrova report` manages the store: its own failures are not recorded in it.
    let records_errors = !matches!(cli.command, Command::Report(_));
    match run(cli, &reports) {
        Ok(code) => code,
        Err(e) => {
            // Our own rendering (not anyhow's): causes deduped, text escaped.
            eprintln!("Error: {}", render::text::error_report(&e));
            if records_errors
                && !interrupt::cancelled_by_user(&e)
                && let Ok(reports) = &reports
            {
                reports.record(&e);
            }
            ExitCode::FAILURE
        }
    }
}

/// Run the parsed command. `reports` is where error reports go
/// (unavailable without a state directory).
fn run(cli: Cli, reports: &Result<Reports, PlatformError>) -> Result<ExitCode> {
    init_tracing(cli.verbose);
    let recipes = recipes::for_this_exe(cli.recipes);
    let mut out = std::io::stdout().lock();
    match cli.command {
        Command::Recipes { command } => recipes::run(&mut out, &recipes, command),
        Command::Scan { json, all } => {
            run_scan(&mut out, &recipes, OutputFormat::from_json_flag(json), all)
        }
        Command::Export(a) => session::export(&mut out, &recipes, cli.verbose, &a),
        Command::Import(a) => session::import(&mut out, &recipes, cli.verbose, &a),
        Command::Send(a) => lan::send(&mut out, &recipes, cli.verbose, &a),
        Command::Receive(a) => lan::receive(&mut out, &recipes, cli.verbose, &a),
        Command::Rollback(a) => session::rollback(&mut out, &recipes, cli.verbose, &a),
        Command::Sessions(a) => session::sessions(&mut out, &recipes, cli.verbose, a),
        Command::Report(a) => {
            let reports = reports
                .as_ref()
                .map_err(|e| anyhow!("error reports are unavailable: {e}"))?;
            report::run(&mut out, reports, a)
        }
    }
}

fn init_tracing(verbose: u8) {
    let level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();
}

/// `carrova scan [--json] [--all]`
fn run_scan(
    out: &mut impl Write,
    recipes: &RecipeLocations,
    format: OutputFormat,
    all: bool,
) -> Result<ExitCode> {
    let recipes = recipes.recipes()?;
    let platform = crate::recipes::seams::platform()?;
    let opts = ScanOptions {
        include_undetected: all,
        ..ScanOptions::default()
    };
    let report = scan(&recipes, platform.as_ref(), opts)?;
    match format {
        OutputFormat::Json => print_json(out, &report)?,
        OutputFormat::Human => render::scan::scan_report(out, &report)?,
    }
    Ok(ExitCode::SUCCESS)
}
