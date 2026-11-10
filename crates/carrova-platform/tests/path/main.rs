//! Lexical path normalisation (`carrova_platform::path`).

use std::path::Path;

use carrova_platform::path::normalize_abs;

#[test]
fn normalize_abs_cases() {
    assert_eq!(normalize_abs(Path::new("/a/./b/../c")), Some("/a/c".into()));
    assert_eq!(normalize_abs(Path::new("/")), Some("/".into()));
    assert_eq!(normalize_abs(Path::new("/..")), None);
    assert_eq!(normalize_abs(Path::new("/a/../..")), None);
    assert_eq!(normalize_abs(Path::new("a/b")), None);
}
