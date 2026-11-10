//! `carrova report` arguments.

use carrova_report::{ReportConsent, ReportId};
use clap::{Args, Subcommand, ValueEnum};

/// `carrova report`
#[derive(Debug, Args)]
pub struct ReportArgs {
    #[command(subcommand)]
    pub(crate) command: Option<ReportCommand>,
    /// Output JSON on stdout.
    #[arg(long, global = true)]
    pub(crate) json: bool,
}

#[derive(Debug, Subcommand)]
pub enum ReportCommand {
    /// List stored reports, newest first (default).
    List,
    /// Show one report in full.
    Show { id: ReportId },
    /// Print the GitHub issue text for one report and the URL that would
    /// file it. Nothing is opened or sent.
    Issue { id: ReportId },
    /// Delete one report. Irreversible.
    Delete {
        id: ReportId,
        /// Don't ask for confirmation.
        #[arg(short, long)]
        yes: bool,
    },
    /// Delete every report (the consent choice stays). Irreversible.
    Clear {
        /// Don't ask for confirmation.
        #[arg(short, long)]
        yes: bool,
    },
    /// Show whether Carrova keeps local error reports, or choose.
    ///
    /// Reports are redacted (no paths, file contents or secrets), stay on
    /// this computer and are only sent if you file one yourself.
    Consent {
        /// `allow` or `deny` (deny deletes stored reports); omit to show
        /// the current choice.
        choice: Option<ConsentChoice>,
        /// Don't ask before deleting stored reports.
        #[arg(short, long)]
        yes: bool,
    },
}

/// The choice `carrova report consent` can set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ConsentChoice {
    /// Keep redacted reports locally.
    Allow,
    /// Record nothing and delete stored reports.
    Deny,
}

impl From<ConsentChoice> for ReportConsent {
    fn from(choice: ConsentChoice) -> Self {
        match choice {
            ConsentChoice::Allow => Self::Allowed,
            ConsentChoice::Deny => Self::Denied,
        }
    }
}
