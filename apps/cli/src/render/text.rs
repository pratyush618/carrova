//! Text primitives shared by every renderer: terminal-safe escaping, error
//! chains, sizes, counts and dates.

use std::borrow::Cow;

use carrova_core::lan::ActionCounts;
use carrova_manifest::is_unsafe_char;

/// `s` made safe for a terminal: control characters (C0, DEL, C1) and
/// Unicode bidi controls are shown escaped (`\u{1b}`), so archive or
/// manifest text cannot move the cursor, erase lines or reorder what is
/// shown. Use for every engine-, archive- or plan-derived string printed.
/// Which characters are unsafe is the manifest's policy
/// ([`is_unsafe_char`]), the same one it validates untrusted text with.
pub fn safe<S: AsRef<str> + ?Sized>(s: &S) -> Cow<'_, str> {
    let s = s.as_ref();
    if !s.chars().any(is_unsafe_char) {
        return Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        if is_unsafe_char(c) {
            out.extend(c.escape_unicode());
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

/// An error and its causes, one per line, escaped with [`safe`]. A cause
/// whose text already appears in the message before it is left out
/// (e.g. `I/O error on X: No such file` followed by `No such file`).
pub fn error_report(err: &anyhow::Error) -> String {
    let mut out = safe(&err.to_string()).into_owned();
    let mut prev = err.to_string();
    let mut header = false;
    for cause in err.chain().skip(1) {
        let text = cause.to_string();
        if !prev.contains(&text) {
            if !header {
                out.push_str("\nCaused by:");
                header = true;
            }
            out.push_str("\n    ");
            out.push_str(&safe(&text));
        }
        prev = text;
    }
    out
}

/// `1500` -> `1.5 kB`: SI units (powers of 1000), like the desktop app,
/// one decimal; a value that would round to `1000.0` moves up a unit.
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["kB", "MB", "GB", "TB", "PB", "EB"];
    /// Smallest value that prints as `1000.0` with one decimal.
    const ROLLS_OVER: f64 = 999.95;
    if bytes < 1000 {
        return format!("{bytes} B");
    }
    #[expect(clippy::cast_precision_loss, reason = "display only")]
    let mut value = bytes as f64 / 1000.0;
    let mut unit = 0;
    while value >= ROLLS_OVER && unit < UNITS.len() - 1 {
        value /= 1000.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

/// `1 file`, `3 files`.
pub fn files(n: u64) -> String {
    if n == 1 {
        "1 file".to_owned()
    } else {
        format!("{n} files")
    }
}

/// `create 1 · replace 2 · keep-both 0 · skip 0 · identical 3`: one
/// item's actions, in the plan view and the sender's plan summary alike.
pub fn action_counts(c: &ActionCounts) -> String {
    format!(
        "create {} \u{b7} replace {} \u{b7} keep-both {} \u{b7} skip {} \u{b7} identical {}",
        c.create, c.replace, c.keep_both, c.skip, c.identical
    )
}

#[cfg(test)]
#[path = "../../tests/render/text/unit.rs"]
mod tests;
