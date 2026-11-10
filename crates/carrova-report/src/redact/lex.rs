//! Splitting text into words and delimiters for [`super::Redactor`].
//!
//! Delimiters (whitespace, quotes, brackets, `,;=|`) are kept verbatim.
//! Two exceptions keep redacted output stable under re-redaction:
//! placeholders (`<path>`) stay inside their word, and a URL
//! (`scheme://…`) stays one word across `=` so its query is seen whole.

/// Characters that separate words; they are copied to the output as-is.
const DELIMITERS: &[char] = &[
    '"', '\'', '`', '(', ')', '[', ']', '{', '}', '<', '>', ',', ';', '=', '|',
];

/// A piece of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Piece<'t> {
    /// Text between delimiters (never empty).
    Word(&'t str),
    /// One delimiter character.
    Delimiter(char),
}

/// Splits `text` into words and delimiters, in order.
pub(crate) fn pieces(text: &str) -> Vec<Piece<'_>> {
    let mut pieces = Vec::new();
    let mut start = 0;
    let mut in_url = false;
    let mut chars = text.char_indices().peekable();
    while let Some((index, ch)) = chars.next() {
        if index == start {
            in_url = scheme_len(&text[index..]).is_some();
        }
        if ch == '<'
            && let Some(len) = placeholder_len(&text[index..])
        {
            // Skip the rest of the placeholder; it belongs to the word.
            while chars.next_if(|&(at, _)| at < index + len).is_some() {}
            continue;
        }
        if ch.is_whitespace() || (DELIMITERS.contains(&ch) && !(in_url && ch == '=')) {
            if start < index {
                pieces.push(Piece::Word(&text[start..index]));
            }
            pieces.push(Piece::Delimiter(ch));
            start = index + ch.len_utf8();
        }
    }
    if start < text.len() {
        pieces.push(Piece::Word(&text[start..]));
    }
    pieces
}

/// Length of a leading `scheme://` (RFC 3986 scheme), if any.
pub(crate) fn scheme_len(text: &str) -> Option<usize> {
    let scheme = text
        .bytes()
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'+' | b'.' | b'-'))
        .count();
    let first_alpha = text.bytes().next().is_some_and(|b| b.is_ascii_alphabetic());
    (first_alpha && text[scheme..].starts_with("://")).then_some(scheme + "://".len())
}

/// Length of a leading placeholder such as `<path>` or `<pairing-code>`.
fn placeholder_len(text: &str) -> Option<usize> {
    let name = text[1..]
        .bytes()
        .take_while(|b| b.is_ascii_lowercase() || *b == b'-')
        .count();
    (name > 0 && text[1 + name..].starts_with('>')).then_some(name + 2)
}
