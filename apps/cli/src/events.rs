//! Renders engine [`Event`]s on stderr: human lines, or one JSON object per
//! line with `--json` (stdout stays reserved for the final JSON result).

use std::collections::HashMap;
use std::io::Write;
use std::sync::{Mutex, OnceLock};

use carrova_core::session::{Event, EventSink, Phase, SessionId};

use crate::output::OutputFormat;
use crate::render::lan as lan_report;
use crate::render::session::rollback_step;
use crate::render::text::{human_size, safe};
use carrova_view::Detail;
use carrova_view::lan::{inventory_view, pause_view, summary_view};

/// Event renderer for the CLI.
#[derive(Debug)]
pub struct CliSink {
    format: OutputFormat,
    verbose: u8,
    /// Last progress quarter printed per phase.
    progress: Mutex<HashMap<Phase, u64>>,
    /// The session, once the command knows it (named in the pause notice).
    session: OnceLock<SessionId>,
}

impl CliSink {
    /// Human lines (`-v` adds detail), or JSON lines.
    pub fn new(format: OutputFormat, verbose: u8) -> Self {
        Self {
            format,
            verbose,
            progress: Mutex::new(HashMap::new()),
            session: OnceLock::new(),
        }
    }

    /// Name `id` in messages that tell the user how to resume. Set once;
    /// later calls are ignored.
    pub fn set_session(&self, id: &SessionId) {
        let _ = self.session.set(id.clone());
    }

    fn human(&self, event: &Event) -> Option<String> {
        let v = self.verbose;
        match event {
            Event::StateChanged { session, from, to } => {
                (v >= 1).then(|| format!("session {session}: {from} -> {to}"))
            }
            Event::Progress { phase, done, total } => self.progress_line(*phase, *done, *total),
            Event::ItemCaptured { item, files, bytes } => Some(format!(
                "captured {}: {files} file(s), {}",
                safe(item),
                human_size(*bytes)
            )),
            Event::Recaptured { path, attempt } => Some(format!(
                "note: {} changed while being read; re-read (attempt {attempt})",
                safe(path)
            )),
            Event::ConflictFound { item, target } => {
                (v >= 1).then(|| format!("conflict: {} ({})", safe(target), safe(item)))
            }
            Event::ActionApplied {
                item,
                target,
                action,
            } => (v >= 1).then(|| format!("{} {} ({})", safe(action), safe(target), safe(item))),
            Event::RollbackStep { target, outcome } => {
                (v >= 1).then(|| rollback_step(target, *outcome))
            }
            Event::Warning { message } => Some(format!("warning: {}", safe(message))),
            // The command renders the checklist itself.
            Event::Finished { .. } => None,
            Event::PeerConnected { peer } => Some(format!("Connected to {peer}.")),
            Event::PeerLost { retrying: true } => {
                Some("Connection lost; reconnecting automatically...".to_owned())
            }
            Event::PeerLost { retrying: false } => Some("Connection lost.".to_owned()),
            Event::InventoryReceived(inv) => Some(lan_report::inventory(&inventory_view(inv))),
            Event::PlanSummaryReceived(s) => Some(lan_report::plan_summary(&summary_view(
                s,
                Detail::from_verbosity(v),
            ))),
            Event::TransferPaused { reason } => {
                pause_view(reason, self.session.get()).map(|v| lan_report::pause(&v))
            }
            Event::TransferResumed => Some("Space available again; transfer resumed.".to_owned()),
            // `carrova send` shows the warning when it asks for a new code;
            // the receiver's command fails with the pairing error.
            Event::PairingFailed => None,
        }
    }

    /// Print at 0/25/50/75/100 % per phase, not on every chunk.
    fn progress_line(&self, phase: Phase, done: u64, total: u64) -> Option<String> {
        if total == 0 {
            return None;
        }
        let quarter = done.min(total).saturating_mul(4) / total;
        let mut last = self.progress.lock().ok()?;
        if last.get(&phase) == Some(&quarter) {
            return None;
        }
        last.insert(phase, quarter);
        Some(format!(
            "{phase}: {}% ({} of {})",
            quarter * 25,
            human_size(done),
            human_size(total)
        ))
    }
}

impl EventSink for CliSink {
    fn emit(&self, event: Event) {
        let line = match self.format {
            OutputFormat::Json => serde_json::to_string(&event).ok(),
            OutputFormat::Human => self.human(&event),
        };
        if let Some(line) = line {
            // Best effort: a closed stderr must not abort the session.
            let _ = writeln!(std::io::stderr().lock(), "{line}");
        }
    }
}
