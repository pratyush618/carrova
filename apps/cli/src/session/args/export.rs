//! `carrova export`: archive file, selection and passphrase protection.

use std::path::PathBuf;

use carrova_core::KdfStrength;
use clap::{Args, ValueEnum};

use super::SelectionArgs;
use crate::prompt::Consent;

/// Archive passphrase protection (`--kdf-strength`); see
/// [`KdfStrength`] for the time and memory each costs.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum KdfStrengthArg {
    /// Suggested: 256 MiB, about a second.
    Standard,
    /// 512 MiB, roughly 2–3× the time.
    Strong,
    /// 1 GiB, roughly 6–8× the time; opening needs that much free RAM.
    Maximum,
}

impl From<KdfStrengthArg> for KdfStrength {
    fn from(k: KdfStrengthArg) -> Self {
        match k {
            KdfStrengthArg::Standard => Self::Standard,
            KdfStrengthArg::Strong => Self::Strong,
            KdfStrengthArg::Maximum => Self::Maximum,
        }
    }
}

/// `carrova export`
#[derive(Debug, Args)]
pub struct ExportArgs {
    /// Archive file to write (must not exist).
    #[arg(short, long, value_name = "FILE")]
    pub(crate) output: PathBuf,
    #[command(flatten)]
    pub(crate) selection: SelectionArgs,
    /// How hard the passphrase is to guess (default standard); stronger
    /// takes longer to write and to open.
    #[arg(long, value_name = "LEVEL", value_enum, conflicts_with = "kdf_memory")]
    pub(crate) kdf_strength: Option<KdfStrengthArg>,
    /// Argon2id memory in MiB (default 256; the engine enforces a floor).
    #[arg(long, value_name = "MIB")]
    pub(crate) kdf_memory: Option<u32>,
    /// Don't ask for confirmation (required when stdin is not a terminal).
    #[arg(short, long)]
    yes: bool,
    /// Events as JSON lines on stderr; final result as JSON on stdout.
    #[arg(long)]
    pub(crate) json: bool,
}

impl ExportArgs {
    /// `--yes`: ask, or agreed up front.
    pub fn consent(&self) -> Consent {
        self.yes.into()
    }
}
