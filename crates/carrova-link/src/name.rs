//! Sanitising of file names received from a peer.

/// Longest name produced, in bytes.
const MAX_NAME: usize = 200;
/// Longest extension (without the dot) kept when shortening; longer "extensions" are just name text.
const MAX_EXT: usize = 16;
/// Name used when nothing usable is left.
const FALLBACK: &str = "file";

/// Reduces a peer-supplied name to one safe path component: no separators or
/// control characters, no leading/trailing dots or spaces, no Windows reserved
/// device name, at most 200 bytes (extension kept), never empty.
#[must_use]
pub fn safe_file_name(raw: &str) -> String {
    let cleaned: String = raw.chars().filter(|c| !is_forbidden(*c)).collect();
    let mut name = cleaned.trim_matches(['.', ' ']).to_owned();
    if name.is_empty() {
        return FALLBACK.to_owned();
    }
    name = cap(name);
    if is_reserved(&name) {
        name.insert(0, '_');
        name = cap(name);
    }
    name
}

/// Separators, controls, Windows-special characters (`:` opens NTFS streams) and bidi overrides.
fn is_forbidden(c: char) -> bool {
    c.is_control()
        || matches!(c, '/' | '\\' | ':' | '<' | '>' | '"' | '|' | '?' | '*')
        || matches!(c, '\u{202A}'..='\u{202E}' | '\u{2066}'..='\u{2069}')
}

/// Whether the part before the first dot, ignoring trailing spaces, is a Windows device name.
fn is_reserved(name: &str) -> bool {
    let stem = name
        .split('.')
        .next()
        .unwrap_or(name)
        .trim_end()
        .to_ascii_uppercase();
    matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) || stem
        .strip_prefix("COM")
        .or_else(|| stem.strip_prefix("LPT"))
        .is_some_and(|n| matches!(n.as_bytes(), [b'1'..=b'9']))
}

/// Shortens the stem to fit [`MAX_NAME`] bytes on a char boundary, keeping the extension.
fn cap(name: String) -> String {
    if name.len() <= MAX_NAME {
        return name;
    }
    let (stem, ext) = match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && ext.len() <= MAX_EXT => {
            (stem, &name[stem.len()..])
        }
        _ => (name.as_str(), ""),
    };
    let end = stem.floor_char_boundary(MAX_NAME - ext.len());
    format!("{}{ext}", stem[..end].trim_end_matches(['.', ' ']))
}
