//! `carrova send`'s terminal side of the engine's pairing prompts.

use std::io::Write;
use std::time::Duration;

use anyhow::Result;
use carrova_core::{CodeOffer, SendPrompt, Ticket};

use crate::output::OutputFormat;
use crate::render::lan::{
    NEW_CODE_NOT_ASKED, NEW_CODE_QUESTION, code_banner, pairing_record, pairing_warning,
};
use carrova_view::lan::code_view;

/// `carrova send`'s side of [`SendPrompt`]: renders codes and the
/// wrong-code warning and asks the user; when to ask and the attempt cap
/// are the engine's. Codes go to `err` as a banner, or as JSON to
/// `out` as the explicit pairing record — the only places a code is
/// written. `ask` is `None` when nobody can answer (no terminal, `--json`):
/// then no new code is offered.
pub struct SendUi<O, E, A> {
    out: O,
    err: E,
    format: OutputFormat,
    ask: Option<A>,
}

impl<O, E, A> SendUi<O, E, A> {
    /// Codes and warnings to `out` / `err` in `format`; `ask` answers the
    /// new-code question (`None`: nobody can).
    pub fn new(out: O, err: E, format: OutputFormat, ask: Option<A>) -> Self {
        Self {
            out,
            err,
            format,
            ask,
        }
    }
}

impl<O: Write, E: Write, A: FnMut(&str) -> Result<bool>> SendUi<O, E, A> {
    fn try_show(
        &mut self,
        ticket: &Ticket,
        offer: CodeOffer,
        expires_in: Duration,
    ) -> std::io::Result<()> {
        let view = code_view(ticket, offer, expires_in);
        match self.format {
            OutputFormat::Json => {
                serde_json::to_writer(&mut self.out, &pairing_record(&view))?;
                writeln!(self.out)?;
                self.out.flush()
            }
            OutputFormat::Human => {
                writeln!(self.err, "{}", code_banner(&view, offer))?;
                self.err.flush()
            }
        }
    }
}

impl<O: Write, E: Write, A: FnMut(&str) -> Result<bool>> SendPrompt for SendUi<O, E, A> {
    fn show_code(&mut self, ticket: &Ticket, offer: CodeOffer, expires_in: Duration) {
        // Best effort: a closed terminal must not end the session.
        let _ = self.try_show(ticket, offer, expires_in);
    }

    fn confirm_new_code(&mut self, failures: u32, remaining: u32) -> bool {
        if self.format == OutputFormat::Human {
            let _ = writeln!(self.err, "{}", pairing_warning(failures, remaining));
        }
        match &mut self.ask {
            // An unreadable answer is a "no".
            Some(ask) => ask(NEW_CODE_QUESTION).unwrap_or(false),
            None => {
                if self.format == OutputFormat::Human {
                    let _ = writeln!(self.err, "{NEW_CODE_NOT_ASKED}");
                }
                false
            }
        }
    }
}

#[cfg(test)]
#[path = "../../tests/lan/send_ui/unit.rs"]
mod tests;
