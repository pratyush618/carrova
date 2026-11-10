//! `carrova rollback` and `carrova sessions`.

use std::path::PathBuf;

use carrova_core::session::SessionId;
use clap::{Args, Subcommand};

use crate::args::parse_session_id;
use crate::prompt::Consent;

/// `carrova rollback`
#[derive(Debug, Args)]
pub struct RollbackArgs {
    /// Import session to undo (default: the latest one that can be rolled back).
    #[arg(value_parser = parse_session_id)]
    pub(crate) session: Option<SessionId>,
    /// Only show what the undo would do; change nothing.
    #[arg(long)]
    pub(crate) preview: bool,
    /// Don't ask for confirmation.
    #[arg(short, long, conflicts_with = "preview")]
    yes: bool,
    /// Result as JSON on stdout.
    #[arg(long)]
    pub(crate) json: bool,
}

impl RollbackArgs {
    /// `--yes`: ask, or agreed up front.
    pub fn consent(&self) -> Consent {
        self.yes.into()
    }
}

/// `carrova sessions`
#[derive(Debug, Args)]
pub struct SessionsArgs {
    #[command(subcommand)]
    pub(crate) command: Option<SessionsCommand>,
    /// Output JSON on stdout.
    #[arg(long, global = true)]
    pub(crate) json: bool,
}

#[derive(Debug, Subcommand)]
pub enum SessionsCommand {
    /// List sessions, newest first (default).
    List,
    /// Show one session.
    Show {
        #[arg(value_parser = parse_session_id)]
        id: SessionId,
    },
    /// Delete a session's rollback point and staging. Irreversible.
    Dismiss {
        #[arg(value_parser = parse_session_id)]
        id: SessionId,
        /// Don't ask for confirmation.
        #[arg(short, long)]
        yes: bool,
    },
    /// Write the session log (sensitive paths redacted) to a file.
    ExportLog {
        #[arg(value_parser = parse_session_id)]
        id: SessionId,
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,
    },
}
