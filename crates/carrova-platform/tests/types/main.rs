//! Platform value types: stable OS ids and process/binary matching.

use std::path::PathBuf;

use carrova_platform::{OsKind, ProcessInfo};

fn proc(name: &str, exe: Option<&str>) -> ProcessInfo {
    ProcessInfo {
        pid: 1,
        name: name.into(),
        exe: exe.map(PathBuf::from),
    }
}

#[test]
fn os_kind_ids_and_display_names_are_stable() {
    let cases = [
        (OsKind::Linux, "linux", "Linux"),
        (OsKind::MacOs, "macos", "macOS"),
        (OsKind::Windows, "windows", "Windows"),
    ];
    for (os, id, name) in cases {
        assert_eq!(os.as_str(), id);
        assert_eq!(os.to_string(), name);
    }
}

#[test]
fn matches_binary_by_name_exe_and_truncation() {
    assert!(proc("git", None).matches_binary("git"));
    assert!(proc("x", Some("/usr/bin/code")).matches_binary("code"));
    assert!(proc("abcdefghijklmno", None).matches_binary("abcdefghijklmnopq"));
    assert!(!proc("abcdefghijklmn", None).matches_binary("abcdefghijklmnopq"));
    assert!(!proc("gitk", None).matches_binary("git"));
}
