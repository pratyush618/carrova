//! The device name shown to the other side of a transfer: one sanitising
//! policy shared by the native adapters and the fake.

/// Longest device name returned, in characters.
pub(crate) const DEVICE_NAME_MAX_CHARS: usize = 64;

/// Bidirectional formatting characters: invisible, and able to make a
/// name render as something else (`Trojan Source`), so never shown.
const BIDI_CONTROLS: [char; 12] = [
    '\u{061C}', '\u{200E}', '\u{200F}', '\u{202A}', '\u{202B}', '\u{202C}', '\u{202D}', '\u{202E}',
    '\u{2066}', '\u{2067}', '\u{2068}', '\u{2069}',
];

fn printable(c: char) -> bool {
    !c.is_control() && !BIDI_CONTROLS.contains(&c)
}

/// `raw` as a displayable device name: non-printable characters removed,
/// surrounding whitespace trimmed, at most [`DEVICE_NAME_MAX_CHARS`]
/// characters; `None` if nothing is left.
pub(crate) fn sanitize_device_name(raw: &str) -> Option<String> {
    let clean: String = raw
        .chars()
        .filter(|&c| printable(c))
        .collect::<String>()
        .trim()
        .chars()
        .take(DEVICE_NAME_MAX_CHARS)
        .collect();
    let name = clean.trim_end();
    (!name.is_empty()).then(|| name.to_owned())
}

#[cfg(test)]
#[path = "../tests/device/unit.rs"]
mod tests;
