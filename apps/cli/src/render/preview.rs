//! What an undo would do (`carrova rollback --preview`, and the question
//! `carrova rollback` asks).

use std::io::{self, Write};

use carrova_core::RollbackPreview;

use super::text::safe;

/// One line of counts: what the undo would restore, remove and move
/// aside, and how many steps need the user's attention.
#[must_use]
pub fn counts(p: &RollbackPreview) -> String {
    format!(
        "{} restored \u{b7} {} removed \u{b7} {} moved aside \u{b7} {} need attention",
        p.restored,
        p.removed,
        p.moved_aside,
        p.blockers.len()
    )
}

/// The preview: counts, then every step the undo would leave in place.
pub fn rollback_preview(out: &mut impl Write, p: &RollbackPreview) -> io::Result<()> {
    writeln!(out, "Undo would do: {}", counts(p))?;
    for b in &p.blockers {
        writeln!(
            out,
            "  needs attention: {}: {}",
            safe(&b.target),
            safe(&b.reason)
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/render/preview/unit.rs"]
mod tests;
