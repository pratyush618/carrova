//! Public-API tests for received file-name sanitising.

use carrova_link::safe_file_name;

#[test]
fn safe_file_name_cases() {
    for (raw, want) in [
        ("../x", "x"),
        ("a/b\\c", "abc"),
        ("con.txt", "_con.txt"),
        ("COM1", "_COM1"),
        ("\0name", "name"),
        ("a\nb\tc", "abc"),
        (".", "file"),
        ("..", "file"),
        ("", "file"),
        ("  .hidden. ", "hidden"),
        ("report.pdf", "report.pdf"),
        ("console.txt", "console.txt"),
        (".pdf", "pdf"),
        ("///", "file"),
        ("a:b", "ab"),
        ("a<b>c\"d|e?f*g", "abcdefg"),
        ("a\u{202E}b\u{2066}c\u{2069}d\u{202A}", "abcd"),
        ("con .txt", "_con .txt"),
        ("CONIN$", "_CONIN$"),
        ("conout$.log", "_conout$.log"),
    ] {
        assert_eq!(safe_file_name(raw), want, "raw = {raw:?}");
    }
}

#[test]
fn long_names_cap_at_200_bytes_keeping_extension() {
    let n = safe_file_name(&format!("{}.pdf", "a".repeat(300)));
    assert_eq!(n.len(), 200);
    assert!(n.ends_with("a.pdf"));
}

#[test]
fn cap_never_splits_a_multibyte_char() {
    // 199 ASCII bytes + a 3-byte char straddles byte 200.
    let n = safe_file_name(&format!("{}€.txt", "a".repeat(198)));
    assert!(n.len() <= 200 && n.ends_with(".txt"));
    let n = safe_file_name(&"€".repeat(100));
    assert!(n.len() <= 200 && n.chars().all(|c| c == '€'));
}

#[test]
fn extension_only_name_is_capped_without_panic() {
    let n = safe_file_name(&format!(".{}", "e".repeat(300)));
    assert_eq!(n, "e".repeat(200));
}

#[test]
fn reserved_prefix_still_fits_the_cap() {
    let n = safe_file_name(&format!("con.{}", "e".repeat(300)));
    assert!(n.len() <= 200 && n.starts_with("_con."));
}
