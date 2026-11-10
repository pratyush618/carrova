//! Terminal-safety of untrusted text: manifest strings end up on screens
//! (the import plan), so characters that move the cursor, clear lines or
//! reorder text are rejected.

use crate::{ManifestError, Result};

/// `true` for C0 controls (incl. `\t`, `\n`, `\r`, ESC), DEL, C1 controls
/// and the Unicode bidi controls (embeddings, overrides, isolates, marks).
#[must_use]
pub fn is_unsafe_char(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}' | '\u{200e}' | '\u{200f}' | '\u{61c}'
        )
}

/// Reject `value` if it holds any [unsafe character](is_unsafe_char). The
/// error carries only an escaped form of the value.
pub(crate) fn check_text(field: &'static str, value: &str) -> Result<()> {
    if value.chars().any(is_unsafe_char) {
        return Err(ManifestError::UnsafeText {
            field,
            escaped: escaped(value),
        });
    }
    Ok(())
}

/// `value` with every non-printable / unsafe character escaped.
pub(crate) fn escaped(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if is_unsafe_char(c) {
                c.escape_unicode().to_string()
            } else {
                c.escape_debug().to_string()
            }
        })
        .collect()
}

#[cfg(test)]
#[path = "../tests/text/unit.rs"]
mod tests;
