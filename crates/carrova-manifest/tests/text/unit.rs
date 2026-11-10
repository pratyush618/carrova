use super::*;

#[test]
fn classifies() {
    for c in [
        '\0', '\t', '\n', '\r', '\x1b', '\x7f', '\u{80}', '\u{9b}', '\u{9f}',
    ] {
        assert!(is_unsafe_char(c), "{c:?}");
    }
    for c in [
        '\u{202a}', '\u{202e}', '\u{2066}', '\u{2069}', '\u{200e}', '\u{200f}', '\u{61c}',
    ] {
        assert!(is_unsafe_char(c), "{c:?}");
    }
    for c in ['a', ' ', '/', 'é', '日', '—', '\u{a0}'] {
        assert!(!is_unsafe_char(c), "{c:?}");
    }
}

#[test]
fn escaped_has_no_unsafe_chars() {
    let e = escaped("a\x1b[2K\u{202e}b\"");
    assert!(!e.chars().any(is_unsafe_char));
    assert_eq!(e, "a\\u{1b}[2K\\u{202e}b\\\"");
}
