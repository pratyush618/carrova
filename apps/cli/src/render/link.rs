//! `carrova link status` and `carrova devices features` text. Device
//! names are peer-provided or typed: escaped with [`safe`].

use std::io::{self, Write};

use carrova_view::ecosystem::FeatureRowView;
use serde::Serialize;

use super::text::safe;
use crate::link::StatusRow;

/// `carrova link status`: one line per known device, then its features.
pub fn status(out: &mut impl Write, rows: &[StatusRow]) -> io::Result<()> {
    if rows.is_empty() {
        return writeln!(out, "No known devices.");
    }
    for row in rows {
        let d = &row.device;
        let ping = row
            .ping_ms
            .map(|ms| format!("  {ms} ms"))
            .unwrap_or_default();
        writeln!(
            out,
            "{}  {}  {}  {}{ping}",
            d.fingerprint,
            safe(&d.name),
            d.os,
            word(d.state)
        )?;
        features(out, &d.features)?;
    }
    Ok(())
}

/// A device's features: this side's setting, the other side's (once its
/// policy arrived) and what actually flows.
pub fn features(out: &mut impl Write, rows: &[FeatureRowView]) -> io::Result<()> {
    for r in rows {
        let remote = r.remote.map_or_else(|| "unknown".to_owned(), word);
        writeln!(
            out,
            "  {:<9}  here {:<3}  there {:<7}  in effect {}",
            word(r.feature),
            word(r.local),
            remote,
            word(r.effective)
        )?;
    }
    Ok(())
}

/// A unit variant's serialized name (`clipboard`, `ask`, `away`).
fn word(value: impl Serialize) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_default()
}
