//! `carrova devices` and `carrova visibility` text. Device names are
//! peer-provided or typed: escaped with [`safe`].

use std::io::{self, Write};

use carrova_core::presence::POLL;
use carrova_view::devices::{KnownDeviceView, VisibilityChoice, VisibilityView};

use super::text::safe;

/// `carrova devices list`: one line per known device, by fingerprint
/// (never the full key).
pub fn device_list(out: &mut impl Write, list: &[KnownDeviceView]) -> io::Result<()> {
    if list.is_empty() {
        return writeln!(out, "No known devices.");
    }
    for d in list {
        let last = d
            .last_move
            .map(|at| format!(", last {}", when(at)))
            .unwrap_or_default();
        writeln!(
            out,
            "{}  {}  {}  {} move(s){last}",
            d.fingerprint,
            safe(&d.name),
            d.os,
            d.moves
        )?;
    }
    Ok(())
}
/// `carrova visibility`: who may find this computer now, and that it only
/// takes effect while the Carrova app runs (the app advertises).
pub fn visibility(out: &mut impl Write, view: VisibilityView) -> io::Result<()> {
    match (view.choice, view.until) {
        (VisibilityChoice::NoOne, _) => writeln!(out, "Visible to no one."),
        (VisibilityChoice::Everyone, Some(until)) => writeln!(
            out,
            "Visible to everyone until {}, then to known devices.",
            when(until)
        ),
        _ => writeln!(out, "Visible to known devices."),
    }?;
    writeln!(
        out,
        "Takes effect within {} seconds while the Carrova app runs on this computer.",
        POLL.as_secs()
    )
}

/// Unix seconds as `YYYYMMDD-HHMMSS UTC` (the session id format).
fn when(secs: u64) -> String {
    format!("{} UTC", carrova_core::time::timestamp(secs))
}
