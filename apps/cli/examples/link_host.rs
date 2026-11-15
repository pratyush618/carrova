//! Test-only stand-in for "the other computer" in `just e2e --link`. The
//! CLI never offers a pair-only code nor answers an ask (the app does), so
//! this does both with the engine: it offers a code, auto-confirms the
//! peer, serves the link supervisor, accepts the one file offered and
//! exits. It also announces itself on the LAN like the app does (the CLI's
//! `link serve` does not), which is how the CLI side finds it. Stdout
//! lines: the ticket, the linked device's id (hex), `ready` (files
//! allowed), the saved file's path.
//! Never packaged: cargo examples are not release artifacts.

use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use carrova_core::ecosystem::{Answer, attach_or_start, offer_pairing};
use carrova_core::presence::Presence;
use carrova_core::{Cancel, Engine, Event, NullSink};
use carrova_link::{Feature, FeatureState};

/// How long to wait for the offered file.
const FILE_WAIT: Duration = Duration::from_secs(90);

/// Print one stdout line now (stdout is a file, not a terminal).
fn say(line: impl std::fmt::Display) {
    let mut out = std::io::stdout();
    // A failed write only means the harness is gone.
    let _ = writeln!(out, "{line}").and_then(|()| out.flush());
}

fn main() -> Result<()> {
    let platform: Arc<dyn carrova_platform::Platform> = Arc::from(carrova_platform::current());
    let engine = || Engine::new(Arc::clone(&platform), Vec::new(), Arc::new(NullSink));
    let record = offer_pairing(
        &engine(),
        &Cancel::default(),
        |invite| say(invite.ticket),
        |_| true,
    )?;
    say(record.public.to_hex());

    let handle = attach_or_start(engine())?;
    let presence = Presence::start(&engine());
    let events = handle.subscribe()?;
    handle.set_feature(&record.public, Feature::Files, FeatureState::Ask)?;
    say("ready");
    loop {
        match events.recv_timeout(FILE_WAIT).context("no file arrived")? {
            Event::AskReceived { ask, .. } => handle.answer(ask, Answer::Accept)?,
            Event::FileSaved { path, .. } => {
                say(path.display());
                presence.stop();
                return Ok(());
            }
            _ => {}
        }
    }
}
