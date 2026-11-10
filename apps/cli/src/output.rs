//! How a command reports its result: human text, or JSON on stdout.

use std::io::Write;

use anyhow::Result;
use serde::Serialize;

/// Output format chosen with `--json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// Human-readable text (events as lines on stderr).
    Human,
    /// Events as JSON lines on stderr; the final result as one JSON object
    /// on stdout.
    Json,
}

impl OutputFormat {
    /// The format selected by a `--json` flag.
    #[must_use]
    pub fn from_json_flag(json: bool) -> Self {
        if json { Self::Json } else { Self::Human }
    }
}

/// A result object on stdout (pretty-printed, newline-terminated).
pub fn print_json(out: &mut impl Write, value: &impl Serialize) -> Result<()> {
    serde_json::to_writer_pretty(&mut *out, value)?;
    writeln!(out)?;
    Ok(())
}
