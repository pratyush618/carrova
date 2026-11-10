//! Lexical path normalisation (`carrova_platform::path`).

use std::path::Path;

use carrova_platform::path::normalize_abs;

#[test]
fn normalize_abs_resolves_dots_lexically() {
    assert_eq!(normalize_abs(Path::new("/a/./b/../c")), Some("/a/c".into()));
    assert_eq!(normalize_abs(Path::new("/")), Some("/".into()));
}

#[test]
fn normalize_abs_refuses_climbing_above_the_root() {
    assert_eq!(normalize_abs(Path::new("/..")), None);
    assert_eq!(normalize_abs(Path::new("/a/../..")), None);
}

#[test]
fn normalize_abs_refuses_relative_paths() {
    assert_eq!(normalize_abs(Path::new("a/b")), None);
    assert_eq!(normalize_abs(Path::new("")), None);
}

#[cfg(windows)]
#[test]
fn normalize_abs_keeps_drive_and_unc_prefixes() {
    assert_eq!(
        normalize_abs(Path::new(r"C:\Users\a\..\b\.\c")),
        Some(r"C:\Users\b\c".into())
    );
    assert_eq!(normalize_abs(Path::new(r"C:\")), Some(r"C:\".into()));
    assert_eq!(
        normalize_abs(Path::new(r"\\server\share\x\..\y")),
        Some(r"\\server\share\y".into())
    );
}

#[cfg(windows)]
#[test]
fn normalize_abs_refuses_rootless_verbatim_and_escaping_windows_paths() {
    assert_eq!(normalize_abs(Path::new(r"C:foo")), None);
    assert_eq!(normalize_abs(Path::new(r"\\?\C:\a")), None);
    assert_eq!(normalize_abs(Path::new(r"\\.\pipe\x")), None);
    assert_eq!(normalize_abs(Path::new(r"C:\..")), None);
}
