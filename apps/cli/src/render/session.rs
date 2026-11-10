//! Text rendering of session results: the end-of-session checklist,
//! rollback results and session records.

use std::io::{self, Write};

use carrova_core::RollbackReport;
use carrova_core::checklist::Checklist;
use carrova_core::session::{Role, RollbackOutcome};
use carrova_core::store::{SessionRecord, Source};
use carrova_core::time::utc_date;

use carrova_view::Detail;

use super::text::safe;

/// Whose checklist is shown, and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChecklistOf<'a> {
    /// This machine's session (`session` id) in `role`.
    Local { session: &'a str, role: Role },
    /// The LAN receiver's checklist, shown on the sender: its rollback
    /// point lives on the other machine.
    Receiver,
}

impl ChecklistOf<'_> {
    /// (done, not done) headings.
    fn headings(self) -> (&'static str, &'static str) {
        match self {
            Self::Local {
                role: Role::Export, ..
            } => ("Exported", "Not exported"),
            Self::Local {
                role: Role::Import, ..
            }
            | Self::Receiver => ("Moved", "Not moved"),
        }
    }
}

/// End-of-session checklist.
pub fn checklist(out: &mut impl Write, c: &Checklist, of: ChecklistOf<'_>) -> io::Result<()> {
    let (done, not_done) = of.headings();
    writeln!(out, "{done} ({}):", c.moved.len())?;
    for m in &c.moved {
        writeln!(out, "  + {}", safe(m))?;
    }
    if !c.skipped.is_empty() {
        writeln!(out, "{not_done} ({}):", c.skipped.len())?;
        for s in &c.skipped {
            writeln!(out, "  - {}: {}", safe(&s.what), safe(&s.reason))?;
        }
    }
    if !c.apps_to_install.is_empty() {
        let there = if of == ChecklistOf::Receiver {
            " on the other machine"
        } else {
            ""
        };
        writeln!(out, "Install these apps{there} to use their data:")?;
        for a in &c.apps_to_install {
            writeln!(out, "  * {}", safe(a))?;
        }
    }
    if c.rollback_available {
        match of {
            ChecklistOf::Local { session, .. } => {
                let session = safe(session);
                writeln!(
                    out,
                    "Rollback point kept: undo with `carrova rollback {session}`, discard with `carrova sessions dismiss {session}`"
                )?;
            }
            ChecklistOf::Receiver => writeln!(
                out,
                "Rollback point kept on the other machine (see `carrova sessions` there)"
            )?,
        }
    }
    Ok(())
}

/// One rollback step (`-v` event line).
pub fn rollback_step(target: &str, outcome: RollbackOutcome) -> String {
    let what = match outcome {
        RollbackOutcome::Restored => "restored",
        RollbackOutcome::Removed => "removed",
        RollbackOutcome::MovedAside => "moved aside (changed since the import)",
        RollbackOutcome::Failed => "FAILED, needs attention:",
    };
    format!("rollback: {what} {}", safe(target))
}

/// Rollback result: counts, then what needs attention (failures, paths
/// moved aside instead of deleted) always listed; restored and removed
/// paths only with [`Detail::Full`] (`-v`).
pub fn rollback(out: &mut impl Write, r: &RollbackReport, detail: Detail) -> io::Result<()> {
    writeln!(
        out,
        "Rolled back: {} restored \u{b7} {} removed \u{b7} {} moved aside \u{b7} {} failed",
        r.restored.len(),
        r.removed.len(),
        r.quarantined.len(),
        r.failed.len()
    )?;
    if detail == Detail::Full {
        for t in &r.restored {
            writeln!(out, "  restored {}", safe(t))?;
        }
        for t in &r.removed {
            writeln!(out, "  removed {}", safe(t))?;
        }
    } else if !(r.restored.is_empty() && r.removed.is_empty()) {
        writeln!(out, "  (-v lists every restored and removed path)")?;
    }
    if !r.quarantined.is_empty() {
        writeln!(
            out,
            "Changed since the import, moved aside instead of deleted ({}):",
            r.quarantined.len()
        )?;
        for q in &r.quarantined {
            writeln!(
                out,
                "  - {} -> {}",
                safe(&q.target),
                safe(&q.moved_to.display().to_string())
            )?;
        }
    }
    if !r.failed.is_empty() {
        writeln!(
            out,
            "Could NOT undo ({}; needs manual attention):",
            r.failed.len()
        )?;
        for f in &r.failed {
            writeln!(out, "  - {}: {}", safe(&f.target), safe(&f.reason))?;
        }
    }
    Ok(())
}

/// One line per session: id, role, state, source (with the peer's OS),
/// rollback mark and when the undo point expires.
pub fn session_list(out: &mut impl Write, list: &[SessionRecord]) -> io::Result<()> {
    if list.is_empty() {
        return writeln!(out, "No sessions.");
    }
    let w = list.iter().map(|s| s.id.0.len()).max().unwrap_or(0);
    for s in list {
        let rb = if s.checklist.as_ref().is_some_and(|c| c.rollback_available) {
            "  [rollback available]"
        } else {
            ""
        };
        let source = s.source.as_ref().map_or_else(String::new, source_label);
        let os = s
            .peer_os
            .map_or_else(String::new, |os| format!(" \u{b7} {os}"));
        let expires = s
            .expires_at
            .map_or_else(String::new, |t| format!("  [undo until {}]", utc_date(t)));
        writeln!(
            out,
            "{:<w$}  {:<6}  {:<12}  {source}{os}{rb}{expires}",
            s.id.0, s.role, s.state
        )?;
    }
    Ok(())
}

/// Hex digits of the peer certificate hash shown in listings.
const SHORT_HASH: usize = 12;

/// One-line description of a session's source for listings: the archive
/// path, or `LAN peer [<name> (]<short cert hash>[)]`.
fn source_label(source: &Source) -> String {
    match source {
        Source::Archive { path, .. } => safe(&path.display().to_string()).into_owned(),
        Source::Lan {
            peer_cert_hash: Some(h),
            peer_name,
            ..
        } => {
            let hex = h.to_string();
            let short = hex.get(..SHORT_HASH).unwrap_or(&hex);
            match peer_name {
                Some(name) => format!("LAN peer {} ({short})", safe(name)),
                None => format!("LAN peer {short}"),
            }
        }
        Source::Lan {
            peer_cert_hash: None,
            ..
        } => "LAN (not paired yet)".to_owned(),
    }
}

/// One session record in full, with its checklist.
pub fn session_show(out: &mut impl Write, s: &SessionRecord) -> io::Result<()> {
    writeln!(out, "Session {}", s.id)?;
    writeln!(out, "  role      {}", s.role)?;
    writeln!(out, "  state     {}", s.state)?;
    writeln!(out, "  created   {} (unix)", s.created_at)?;
    writeln!(out, "  updated   {} (unix)", s.updated_at)?;
    match &s.source {
        Some(Source::Archive { path, .. }) => {
            writeln!(out, "  archive   {}", safe(&path.display().to_string()))?;
        }
        Some(Source::Lan { peer_cert_hash, .. }) => {
            let peer = peer_cert_hash.map_or_else(|| "not paired".to_owned(), |h| h.to_string());
            writeln!(out, "  lan peer  {peer}")?;
        }
        None => {}
    }
    if s.role == Role::Import {
        writeln!(out, "  approved  {}", if s.approved { "yes" } else { "no" })?;
    }
    if let Some(e) = &s.error {
        writeln!(out, "  error     {}", safe(e))?;
    }
    if let Some(c) = &s.checklist {
        checklist(
            out,
            c,
            ChecklistOf::Local {
                session: &s.id.0,
                role: s.role,
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/render/session/unit.rs"]
mod tests;
