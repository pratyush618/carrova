//! Terminal interaction: confirmations, the archive passphrase and the
//! pairing code.

use std::io::{self, BufRead, IsTerminal, Write};

use anyhow::{Context, Result, bail};
use carrova_core::{Passphrase, Ticket};
use carrova_view::passphrase::{PassphraseUse, archive_passphrase};
use zeroize::Zeroizing;

/// Env var holding the archive passphrase, for automation and tests. When
/// set, Carrova never prompts. Prefer the prompt for interactive use: the
/// environment of a process can be visible to other local users/tools.
pub const PASSPHRASE_ENV: &str = "CARROVA_PASSPHRASE";

/// Whether the user is asked before something happens, or has agreed
/// up front (`--yes`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consent {
    /// Ask on the terminal.
    Ask,
    /// Agreed up front: `--yes`.
    Assume,
}

impl From<bool> for Consent {
    /// The `--yes` flag.
    fn from(yes: bool) -> Self {
        if yes { Self::Assume } else { Self::Ask }
    }
}

/// Fail early (before any work) when a confirmation will be needed but
/// cannot be asked.
pub fn ensure_can_confirm(consent: impl Into<Consent>, what: &str) -> Result<()> {
    if consent.into() == Consent::Assume || io::stdin().is_terminal() {
        Ok(())
    } else {
        bail!(
            "{what} needs your confirmation, but stdin is not a terminal; \
             re-run with --yes to confirm non-interactively"
        )
    }
}

/// Ask `question` on stderr; [`Consent::Assume`] (`--yes`) answers it.
/// Only `y`/`yes` confirms.
pub fn confirm(consent: impl Into<Consent>, question: &str) -> Result<bool> {
    if consent.into() == Consent::Assume {
        return Ok(true);
    }
    let mut err = io::stderr().lock();
    write!(err, "{question} [y/N] ")?;
    err.flush()?;
    drop(err);
    let mut line = String::new();
    io::stdin().lock().read_line(&mut line)?;
    Ok(matches!(
        line.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

/// Archive passphrase: `CARROVA_PASSPHRASE` if set, else a no-echo terminal
/// prompt (asked twice for a new archive). Never echoed, logged or
/// printed. The shared [`carrova_view::passphrase`] rule decides what is
/// acceptable (a new archive refuses an empty or weak passphrase).
pub fn passphrase(purpose: PassphraseUse) -> Result<Passphrase> {
    if let Some(value) = std::env::var_os(PASSPHRASE_ENV) {
        let value = value
            .into_string()
            .map_err(|_| anyhow::anyhow!("{PASSPHRASE_ENV} is not valid UTF-8"))?;
        return archive_passphrase(value, purpose)
            .map_err(|refused| anyhow::anyhow!("{PASSPHRASE_ENV} refused: {refused}"));
    }
    let pass = archive_passphrase(prompt_passphrase("Archive passphrase: ")?, purpose)
        .map_err(|refused| anyhow::anyhow!("passphrase refused: {refused}"))?;
    if purpose == PassphraseUse::NewArchive {
        let again = Zeroizing::new(prompt_passphrase("Repeat passphrase: ")?);
        if pass.expose() != again.as_str() {
            bail!("passphrases do not match");
        }
    }
    Ok(pass)
}

/// One no-echo passphrase prompt on the terminal.
fn prompt_passphrase(prompt: &str) -> Result<String> {
    rpassword::prompt_password(prompt).with_context(|| {
        format!("cannot prompt for the passphrase (no terminal); set {PASSPHRASE_ENV}")
    })
}

/// Ask for the code shown on the sending machine (visible typing: the code
/// is short-lived and single-use, so seeing typos matters more). The line
/// is zeroized after parsing; the code is never logged or echoed back.
pub fn pairing_ticket() -> Result<Ticket> {
    if !io::stdin().is_terminal() {
        bail!("no code given and stdin is not a terminal; pass it: `carrova receive <code>`");
    }
    let mut err = io::stderr().lock();
    write!(err, "Code shown on the other machine: ")?;
    err.flush()?;
    drop(err);
    let mut line = Zeroizing::new(String::new());
    io::stdin().lock().read_line(&mut line)?;
    crate::args::parse_ticket(&line).map_err(|e| anyhow::anyhow!(e))
}
