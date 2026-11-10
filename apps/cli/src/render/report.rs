//! `carrova report` text: the stored error reports, one report, its
//! GitHub issue draft and the consent choice. Every report string is
//! escaped with [`safe`] (redacted, but still error text).

use std::io::{self, Write};

use carrova_report::{ErrorReport, IssueDraft, ReportConsent, ReportId, ReportKind, ReportSummary};

use super::text::safe;

/// How to turn reporting on, shown wherever it is off.
const TURN_ON: &str = "Turn them on with `carrova report consent allow`.";

/// `carrova report list`: one line per report, newest first, plus a hint
/// when reporting is off.
pub fn report_list(
    out: &mut impl Write,
    list: &[ReportSummary],
    consent: ReportConsent,
) -> io::Result<()> {
    if list.is_empty() {
        writeln!(out, "No error reports.")?;
    }
    for r in list {
        writeln!(
            out,
            "{}  {}  {:<5}  {}",
            r.id,
            when(r.when),
            kind(r.kind),
            safe(&r.headline)
        )?;
    }
    if !consent.is_allowed() {
        writeln!(out, "Error reports are off. {TURN_ON}")?;
    }
    Ok(())
}

/// `carrova report show ID`: everything the report holds.
pub fn report_show(out: &mut impl Write, id: &ReportId, r: &ErrorReport) -> io::Result<()> {
    writeln!(out, "Report {id} ({})", kind(r.kind))?;
    writeln!(
        out,
        "  Carrova {} on {}, {}",
        safe(&r.app_version),
        safe(&r.os),
        when(r.when)
    )?;
    writeln!(out, "Messages:")?;
    for message in &r.message_chain {
        writeln!(out, "  - {}", safe(message))?;
    }
    if !r.backtrace_frames.is_empty() {
        writeln!(out, "Carrova frames:")?;
        for frame in &r.backtrace_frames {
            writeln!(out, "  {}", safe(frame))?;
        }
    }
    writeln!(
        out,
        "To report it on GitHub, review the text from `carrova report issue {id}`."
    )
}

/// `carrova report issue ID`: the exact title and body, then the URL that
/// would file them. Nothing is opened or sent.
pub fn issue(out: &mut impl Write, draft: &IssueDraft) -> io::Result<()> {
    writeln!(out, "Title: {}", safe(&draft.title))?;
    writeln!(out, "Body:")?;
    for line in draft.body.lines() {
        writeln!(out, "{}", safe(line))?;
    }
    writeln!(out)?;
    writeln!(
        out,
        "Nothing has been sent. To file this issue, open this URL in your browser:"
    )?;
    writeln!(out, "{}", safe(&draft.url))
}

/// `carrova report consent`: the current choice and what it means.
pub fn consent(out: &mut impl Write, consent: ReportConsent) -> io::Result<()> {
    match consent {
        ReportConsent::Allowed => writeln!(
            out,
            "Error reports: on. Redacted reports of crashes and errors are kept on this \
             computer; nothing is sent unless you file one yourself (`carrova report issue ID`)."
        ),
        ReportConsent::Denied => {
            writeln!(out, "Error reports: off. Nothing is recorded. {TURN_ON}")
        }
        ReportConsent::Unset => writeln!(
            out,
            "Error reports: not chosen yet, so off. Nothing is recorded. {TURN_ON}"
        ),
    }
}

/// Creation time as `YYYYMMDD-HHMMSS UTC` (the session id format).
fn when(secs: u64) -> String {
    format!("{} UTC", carrova_core::time::timestamp(secs))
}

fn kind(kind: ReportKind) -> &'static str {
    match kind {
        ReportKind::Panic => "crash",
        ReportKind::Error => "error",
    }
}
