//! `RelPath`: accepted and rejected relative paths, `./` stripping and components.

use carrova_manifest::RelPath;

#[test]
fn accepts_normal_paths() {
    for p in [
        "git/config",
        ".ssh/id_ed25519",
        "a",
        "a b/c.d",
        "..hidden/x..y",
    ] {
        assert_eq!(RelPath::new(p).unwrap().as_str(), p);
    }
}

#[test]
fn strips_leading_dot_slash_only_in_new() {
    assert_eq!(
        RelPath::new("././git/config").unwrap().as_str(),
        "git/config"
    );
    assert!(RelPath::try_from("./git".to_owned()).is_err());
}

#[test]
fn rejects_bad_paths() {
    for p in [
        "",
        "/etc/passwd",
        "..",
        "../x",
        "a/../b",
        "a/..",
        "a//b",
        "a/",
        "a/./b",
        ".",
        "./",
        "a\0b",
        "a\x1b[1Ab",
        "a\nb",
        "a\u{202e}b",
    ] {
        assert!(RelPath::new(p).is_err(), "{p:?} should be rejected");
    }
}

#[test]
fn components_split() {
    let p = RelPath::new("a/b/c").unwrap();
    assert_eq!(p.components().collect::<Vec<_>>(), ["a", "b", "c"]);
}
