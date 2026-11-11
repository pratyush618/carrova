//! `carrova integrate`: the "Send with Carrova" entries in the file
//! manager's right-click menu, the same switch as the desktop app's
//! Settings. Writing them is the platform's job; this only asks for it.

use std::io::Write;
use std::process::ExitCode;

use anyhow::{Result, anyhow};
use carrova_platform::FileManagerStatus;
use clap::{Subcommand, ValueEnum};

use crate::recipes::seams;
use crate::render::text::safe;

/// The executable the entries launch: the desktop app, never this CLI
/// (it opens the Send screen with the chosen paths).
const DESKTOP_APP: &str = "carrova-desktop";

/// `carrova integrate`
#[derive(Debug, Subcommand)]
pub enum IntegrateCommand {
    /// "Send with Carrova" in the file manager's right-click menu (this
    /// user only; needs the desktop app).
    FileManager {
        /// Add the entries, remove every one Carrova wrote, or show whether
        /// they are there.
        #[arg(value_enum)]
        state: OnOffStatus,
    },
}

/// `on`, `off` or `status`.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OnOffStatus {
    /// Add (or refresh) the entries.
    On,
    /// Remove the entries Carrova wrote, and only those.
    Off,
    /// Show whether the entries are there.
    Status,
}

/// `carrova integrate file-manager on|off|status`: prints `on`, `off` or
/// `not supported here`, then the file managers left untouched.
pub fn integrate(out: &mut impl Write, cmd: &IntegrateCommand) -> Result<ExitCode> {
    let IntegrateCommand::FileManager { state } = cmd;
    let platform = seams::platform()?;
    let current = platform.file_manager_status()?;
    if !current.supported {
        writeln!(out, "not supported here")?;
        return Ok(match state {
            OnOffStatus::Status => ExitCode::SUCCESS,
            OnOffStatus::On | OnOffStatus::Off => ExitCode::FAILURE,
        });
    }
    let status = match state {
        OnOffStatus::Status => current,
        OnOffStatus::Off => platform.remove_file_manager_entries()?,
        OnOffStatus::On => {
            let launcher = platform.find_binary(DESKTOP_APP)?.ok_or_else(|| {
                anyhow!(
                    "the Carrova desktop app ({DESKTOP_APP}) is not on your PATH; turn this on \
                     in the app instead: Settings → Sending"
                )
            })?;
            platform.install_file_manager_entries(&launcher)?
        }
    };
    report(out, &status)?;
    Ok(ExitCode::SUCCESS)
}

fn report(out: &mut impl Write, s: &FileManagerStatus) -> std::io::Result<()> {
    writeln!(out, "{}", if s.installed { "on" } else { "off" })?;
    for skipped in &s.skipped {
        writeln!(out, "  {}", safe(skipped))?;
    }
    Ok(())
}
