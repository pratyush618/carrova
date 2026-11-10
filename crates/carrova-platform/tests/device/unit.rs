use super::*;

#[test]
fn plain_hostname_is_kept() {
    assert_eq!(
        sanitize_device_name("ezio-laptop").as_deref(),
        Some("ezio-laptop")
    );
}

#[test]
fn surrounding_whitespace_and_newline_are_trimmed() {
    assert_eq!(sanitize_device_name("  desk\n").as_deref(), Some("desk"));
}

#[test]
fn control_and_bidi_characters_are_removed() {
    assert_eq!(
        sanitize_device_name("a\u{0}b\u{1b}[31mc\u{202E}d\u{7f}").as_deref(),
        Some("ab[31mcd")
    );
}

#[test]
fn inner_spaces_and_unicode_survive() {
    assert_eq!(
        sanitize_device_name("Zoë's MacBook").as_deref(),
        Some("Zoë's MacBook")
    );
}

#[test]
fn long_names_are_cut_to_the_character_limit() {
    let name = sanitize_device_name(&"é".repeat(100)).unwrap();
    assert_eq!(name.chars().count(), DEVICE_NAME_MAX_CHARS);
}

#[test]
fn cut_never_leaves_trailing_whitespace() {
    let raw = format!("{} tail", "x".repeat(DEVICE_NAME_MAX_CHARS - 1));
    assert_eq!(
        sanitize_device_name(&raw),
        Some("x".repeat(DEVICE_NAME_MAX_CHARS - 1))
    );
}

#[test]
fn empty_or_unprintable_names_are_none() {
    for raw in ["", "   ", "\n\t", "\u{202E}\u{0}"] {
        assert_eq!(sanitize_device_name(raw), None, "{raw:?}");
    }
}
