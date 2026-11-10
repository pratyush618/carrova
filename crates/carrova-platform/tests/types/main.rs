//! Platform value types: process/binary matching.

use std::path::PathBuf;

use carrova_platform::ProcessInfo;

fn proc(name: &str, exe: Option<&str>) -> ProcessInfo {
    ProcessInfo {
        pid: 1,
        name: name.into(),
        exe: exe.map(PathBuf::from),
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
