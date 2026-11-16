//! Phone numbers. They span several words (`+91 98765 43210`,
//! `(415) 555-0100`), so they are found in the whole text before it is
//! split into words.

/// Placeholder for a phone number.
pub(super) const PLACEHOLDER: &str = "<phone>";
/// Digit count of a phone number (national number up to E.164's 15).
const DIGITS: std::ops::RangeInclusive<usize> = 10..=15;

/// Replaces every phone number in `text` with [`PLACEHOLDER`].
///
/// A number is 10–15 digits, optionally led by `+` or `(`, grouped by single
/// spaces, dashes or parentheses, and not part of a longer word or dotted
/// number (versions, IPv4, ids, floats). Shorter numbers stay readable.
// shortcut: bare 10-digit numbers (epoch seconds) are redacted too, indistinguishable from a phone number.
pub(super) fn redact(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut at = 0;
    while at < chars.len() {
        if let Some(end) = number_end(&chars, at) {
            out.push_str(PLACEHOLDER);
            at = end;
        } else {
            out.push(chars[at]);
            at += 1;
        }
    }
    out
}

/// End (exclusive) of the phone number starting at `start`, if one does.
fn number_end(chars: &[char], start: usize) -> Option<usize> {
    if !matches!(chars[start], '+' | '(' | '0'..='9') || !boundary_before(chars, start) {
        return None;
    }
    let (mut digits, mut end) = (0, start);
    let mut at = start;
    while let Some(&ch) = chars.get(at) {
        match ch {
            '0'..='9' => {
                digits += 1;
                end = at + 1;
                if digits > *DIGITS.end() {
                    break; // too long: stops long digit runs rescanning quadratically
                }
            }
            '+' | '(' if at == start => {}
            ' ' | '-' | '(' | ')' if at > start && !matches!(chars[at - 1], ' ' | '-') => {}
            _ => break,
        }
        at += 1;
    }
    (DIGITS.contains(&digits) && boundary_after(chars, end)).then_some(end)
}

/// `true` unless the number would continue a word, path or dotted number.
fn boundary_before(chars: &[char], start: usize) -> bool {
    start == 0 || !(chars[start - 1].is_alphanumeric() || "_.-/:".contains(chars[start - 1]))
}

fn boundary_after(chars: &[char], end: usize) -> bool {
    match chars.get(end) {
        None => true,
        Some(&ch) if ch.is_alphanumeric() || ch == '_' => false,
        Some('.' | '-' | '/' | ':') => !chars.get(end + 1).is_some_and(|c| c.is_alphanumeric()),
        Some(_) => true,
    }
}
