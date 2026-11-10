//! `RelPath`: accepted forms and rejected components.

use std::path::Path;

use carrova_recipe::{RelPath, RelPathError};

#[test]
fn accepts_and_normalises() {
    let cases = [
        (".gitconfig", ".gitconfig"),
        ("git", "git"),
        ("git/config", "git/config"),
        ("./git", "git"),
        ("git/./config", "git/config"),
        ("git//config/", "git/config"),
        ("a/.../b", "a/.../b"),
        ("..foo", "..foo"),
        ("foo..", "foo.."),
        ("Code/User/settings.json", "Code/User/settings.json"),
    ];
    for (raw, want) in cases {
        let got = RelPath::new(raw).map(|p| p.as_str().to_owned());
        assert_eq!(got.as_deref(), Ok(want), "input {raw:?}");
    }
}

#[test]
fn rejects_bad_paths() {
    use RelPathError as E;
    type Check = fn(&RelPathError) -> bool;
    let cases: &[(&str, Check)] = &[
        ("", |e| matches!(e, E::Empty)),
        (".", |e| matches!(e, E::Empty)),
        ("./", |e| matches!(e, E::Empty)),
        ("././/.", |e| matches!(e, E::Empty)),
        ("/", |e| matches!(e, E::Absolute(_))),
        ("/etc/passwd", |e| matches!(e, E::Absolute(_))),
        ("//server/share", |e| matches!(e, E::Absolute(_))),
        ("~", |e| matches!(e, E::Tilde(_))),
        ("~/.ssh", |e| matches!(e, E::Tilde(_))),
        ("~root/.ssh", |e| matches!(e, E::Tilde(_))),
        ("..", |e| matches!(e, E::ParentComponent(_))),
        ("../x", |e| matches!(e, E::ParentComponent(_))),
        ("a/../../x", |e| matches!(e, E::ParentComponent(_))),
        ("a/..", |e| matches!(e, E::ParentComponent(_))),
        ("./..", |e| matches!(e, E::ParentComponent(_))),
        ("a\\b", |e| matches!(e, E::Backslash(_))),
        ("..\\x", |e| matches!(e, E::Backslash(_))),
        ("C:\\Windows", |e| matches!(e, E::Backslash(_))),
        ("C:/Windows", |e| matches!(e, E::Colon(_))),
        ("c:", |e| matches!(e, E::Colon(_))),
        ("a/C:/x", |e| matches!(e, E::Colon(_))),
        ("my:file", |e| matches!(e, E::Colon(_))),
        ("a\0b", |e| matches!(e, E::Nul)),
    ];
    for (raw, ok) in cases {
        match RelPath::new(raw) {
            Ok(p) => panic!("{raw:?} accepted as {p:?}"),
            Err(e) => assert!(ok(&e), "{raw:?} gave unexpected error {e:?}"),
        }
    }
}

#[test]
fn deserialize_validates() {
    let ok: RelPath = serde_yaml_ng::from_str("./git//config").unwrap();
    assert_eq!(ok.as_str(), "git/config");
    assert!(serde_yaml_ng::from_str::<RelPath>("../etc").is_err());
    assert_eq!(serde_yaml_ng::to_string(&ok).unwrap().trim(), "git/config");
}

#[test]
fn join_to_base() {
    let p = RelPath::new("a/b").unwrap();
    assert_eq!(p.join_to(Path::new("/home/u")), Path::new("/home/u/a/b"));
}

#[test]
fn windows_accepts_backslash_and_normalises() {
    let cases = [
        ("AppData\\Code\\User", "AppData/Code/User"),
        ("Code/User\\settings.json", "Code/User/settings.json"),
        (".\\Git\\\\config\\", "Git/config"),
    ];
    for (raw, want) in cases {
        let got = RelPath::new_windows(raw).map(|p| p.as_str().to_owned());
        assert_eq!(got.as_deref(), Ok(want), "input {raw:?}");
    }
}

#[test]
fn windows_rejects_absolute_and_escapes() {
    use RelPathError as E;
    type Check = fn(&RelPathError) -> bool;
    let cases: &[(&str, Check)] = &[
        ("C:\\x", |e| matches!(e, E::Colon(_))),
        ("c:x", |e| matches!(e, E::Colon(_))),
        ("AppData\\C:\\x", |e| matches!(e, E::Colon(_))),
        ("a/file:stream", |e| matches!(e, E::Colon(_))),
        ("\\\\srv\\x", |e| matches!(e, E::Absolute(_))),
        ("\\x", |e| matches!(e, E::Absolute(_))),
        ("/x", |e| matches!(e, E::Absolute(_))),
        ("..\\x", |e| matches!(e, E::ParentComponent(_))),
        ("a\\..\\..\\x", |e| matches!(e, E::ParentComponent(_))),
        (".\\", |e| matches!(e, E::Empty)),
        ("~\\x", |e| matches!(e, E::Tilde(_))),
    ];
    for (raw, ok) in cases {
        match RelPath::new_windows(raw) {
            Ok(p) => panic!("{raw:?} accepted as {p:?}"),
            Err(e) => assert!(ok(&e), "{raw:?} gave unexpected error {e:?}"),
        }
    }
}

#[test]
fn absolute_errors_point_to_semantic_roots() {
    for raw in ["C:\\x", "AppData\\C:\\x", "\\\\srv\\x"] {
        let msg = RelPath::new_windows(raw).unwrap_err().to_string();
        assert!(msg.contains("root: user_config"), "{msg}");
    }
}

#[test]
fn raw_paths_allow_colons_but_stay_confined() {
    assert_eq!(RelPath::new_raw("a:b/c").unwrap().as_str(), "a:b/c");
    assert!(RelPath::new("a:b/c").is_err());
    for raw in ["../x", "a/../../x", "/x", "~/x", "a\\b"] {
        assert!(RelPath::new_raw(raw).is_err(), "{raw:?} accepted");
    }
}
