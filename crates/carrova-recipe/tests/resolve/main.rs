//! Item resolution: globs, excludes, symlinks and skip reasons.

#![allow(clippy::unwrap_used)]

#[path = "../support/mod.rs"]
mod support;

use std::fs;
use std::path::Path;

use carrova_platform::fake::FakePlatform;
use carrova_platform::{FileKind, OsKind};
use carrova_recipe::{
    Flags, Item, OsOverrides, OsPaths, PathRef, RelPath, ResolvedItem, SemanticRoot, SkipReason,
    Skipped, resolve_item,
};
use support::symlink;

fn pr(root: SemanticRoot, rel: &str) -> PathRef {
    PathRef {
        root,
        rel: RelPath::new(rel).unwrap(),
    }
}

fn item(paths: Vec<PathRef>, exclude: &[&str]) -> Item {
    Item {
        id: "it".into(),
        label: "It".into(),
        paths,
        exclude: exclude.iter().map(|s| (*s).to_owned()).collect(),
        flags: Flags::default(),
        unmovable: None,
        handler: None,
        os: None,
    }
}

fn write(path: &Path, bytes: usize) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, vec![b'x'; bytes]).unwrap();
}

fn rels(r: &ResolvedItem) -> Vec<(SemanticRoot, &str, FileKind)> {
    r.entries
        .iter()
        .map(|e| (e.root, e.rel.as_str(), e.kind))
        .collect()
}

#[test]
fn walks_nested_dirs_with_excludes() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(&home.join(".config/git/config"), 10);
    write(&home.join(".config/git/credentials"), 99);
    write(&home.join(".config/git/hooks/pre-commit"), 5);
    write(&home.join(".config/git/hooks/sub/credentials"), 7);
    write(&home.join(".config/git/cache/blob"), 1000);
    write(&home.join(".config/git/x.log"), 3);
    write(&home.join(".config/git/deep/a.log"), 4);
    write(&home.join(".gitconfig"), 20);
    let it = item(
        vec![
            pr(SemanticRoot::Home, ".gitconfig"),
            pr(SemanticRoot::UserConfig, "git"),
        ],
        // item-relative, root-relative, and single-segment `*`
        &["**/credentials", "git/cache", "*.log"],
    );
    let r = resolve_item(&it, &FakePlatform::with_home(home));
    assert_eq!(
        rels(&r),
        vec![
            (SemanticRoot::UserConfig, "git", FileKind::Dir),
            (SemanticRoot::UserConfig, "git/config", FileKind::File),
            (SemanticRoot::UserConfig, "git/deep", FileKind::Dir),
            (SemanticRoot::UserConfig, "git/deep/a.log", FileKind::File),
            (SemanticRoot::UserConfig, "git/hooks", FileKind::Dir),
            (
                SemanticRoot::UserConfig,
                "git/hooks/pre-commit",
                FileKind::File
            ),
            (SemanticRoot::UserConfig, "git/hooks/sub", FileKind::Dir),
            (SemanticRoot::Home, ".gitconfig", FileKind::File),
        ]
    );
    assert_eq!(r.total_size, 10 + 4 + 5 + 20);
    assert!(r.missing.is_empty());
    assert!(r.skipped.is_empty(), "{:?}", r.skipped);
    assert!(r.entries.iter().all(|e| e.mode.is_some()));
}

#[test]
fn missing_paths_are_reported() {
    let tmp = tempfile::tempdir().unwrap();
    let it = item(
        vec![
            pr(SemanticRoot::Home, ".nope"),
            pr(SemanticRoot::UserConfig, "a/b/c"),
        ],
        &[],
    );
    let r = resolve_item(&it, &FakePlatform::with_home(tmp.path()));
    assert!(r.entries.is_empty());
    assert_eq!(r.total_size, 0);
    assert_eq!(r.missing, it.paths);
}

#[test]
fn symlink_inside_roots_kept_as_symlink() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(&home.join("dotfiles/zshrc"), 8);
    write(&home.join(".config/app/real.conf"), 2);
    symlink("dotfiles/zshrc", &home.join(".zshrc"));
    symlink("real.conf", &home.join(".config/app/link.conf"));
    // directory symlink inside root: recorded, never descended
    symlink("../../dotfiles", &home.join(".config/app/dots"));
    let it = item(
        vec![
            pr(SemanticRoot::Home, ".zshrc"),
            pr(SemanticRoot::UserConfig, "app"),
        ],
        &[],
    );
    let r = resolve_item(&it, &FakePlatform::with_home(home));
    assert_eq!(
        rels(&r),
        vec![
            (SemanticRoot::UserConfig, "app", FileKind::Dir),
            (SemanticRoot::UserConfig, "app/dots", FileKind::Symlink),
            (SemanticRoot::UserConfig, "app/link.conf", FileKind::Symlink),
            (SemanticRoot::UserConfig, "app/real.conf", FileKind::File),
            (SemanticRoot::Home, ".zshrc", FileKind::Symlink),
        ]
    );
    let zshrc = &r.entries[4];
    assert_eq!(
        zshrc.link_target.as_deref(),
        Some(Path::new("dotfiles/zshrc"))
    );
    assert_eq!(r.total_size, 2, "symlink sizes not counted");
    assert!(r.skipped.is_empty(), "{:?}", r.skipped);
}

#[test]
fn symlink_escaping_roots_is_skipped() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let outside = tmp.path().join("outside");
    write(&outside.join("secret"), 50);
    write(&home.join(".config/app/ok"), 1);
    symlink(outside.join("secret"), &home.join(".config/app/abs"));
    symlink("../../../outside/secret", &home.join(".config/app/rel"));
    symlink("/etc/passwd", &home.join(".app-top"));
    let it = item(
        vec![
            pr(SemanticRoot::UserConfig, "app"),
            pr(SemanticRoot::Home, ".app-top"),
        ],
        &[],
    );
    let r = resolve_item(&it, &FakePlatform::with_home(&home));
    assert_eq!(
        rels(&r),
        vec![
            (SemanticRoot::UserConfig, "app", FileKind::Dir),
            (SemanticRoot::UserConfig, "app/ok", FileKind::File),
        ]
    );
    let skipped: Vec<(&str, bool)> = r
        .skipped
        .iter()
        .map(|s| {
            (
                s.rel.as_str(),
                matches!(s.reason, SkipReason::SymlinkEscapesRoots { .. }),
            )
        })
        .collect();
    assert_eq!(
        skipped,
        vec![("app/abs", true), ("app/rel", true), (".app-top", true)]
    );
}

#[test]
fn symlink_allowed_roots_are_own_root_and_home_only() {
    // Regression: a symlink counted as non-escaping if its target lay
    // under ANY resolved root, so one bogus root (here Documents = `/`)
    // let every symlink through.
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path().join("home");
    let data = tmp.path().join("data");
    write(&data.join("blob"), 1);
    write(&home.join(".config/app/ok"), 1);
    symlink("/etc/passwd", &home.join(".app-top"));
    // user_data lives outside home: not an allowed target for a user_config item
    symlink(data.join("blob"), &home.join(".config/app/to-data"));
    // home is always allowed
    symlink("../../.app-top", &home.join(".config/app/to-home"));
    let it = item(
        vec![
            pr(SemanticRoot::Home, ".app-top"),
            pr(SemanticRoot::UserConfig, "app"),
        ],
        &[],
    );
    let p = FakePlatform::with_home(&home)
        .root(SemanticRoot::Documents, "/")
        .root(SemanticRoot::UserData, &data);
    let r = resolve_item(&it, &p);
    assert_eq!(
        rels(&r),
        vec![
            (SemanticRoot::UserConfig, "app", FileKind::Dir),
            (SemanticRoot::UserConfig, "app/ok", FileKind::File),
            (SemanticRoot::UserConfig, "app/to-home", FileKind::Symlink),
        ]
    );
    let escapes: Vec<&str> = r
        .skipped
        .iter()
        .filter(|s| matches!(s.reason, SkipReason::SymlinkEscapesRoots { .. }))
        .map(|s| s.rel.as_str())
        .collect();
    assert_eq!(escapes, vec!["app/to-data", ".app-top"]);
}

#[test]
fn filesystem_root_as_semantic_root_is_unavailable() {
    let tmp = tempfile::tempdir().unwrap();
    let it = item(vec![pr(SemanticRoot::Documents, "etc/passwd")], &[]);
    for bogus in ["/", "/tmp/.."] {
        let p = FakePlatform::with_home(tmp.path()).root(SemanticRoot::Documents, bogus);
        let r = resolve_item(&it, &p);
        assert!(r.entries.is_empty(), "{bogus}: {:?}", r.entries);
        assert!(r.missing.is_empty());
        assert_eq!(r.skipped.len(), 1);
        assert!(
            matches!(r.skipped[0].reason, SkipReason::RootUnavailable(_)),
            "{bogus}: {:?}",
            r.skipped
        );
    }
}

#[test]
fn symlinked_parent_is_not_followed() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(&home.join("real/app/conf"), 1);
    fs::create_dir_all(home.join(".config")).unwrap();
    symlink("../real/app", &home.join(".config/app"));
    let it = item(vec![pr(SemanticRoot::UserConfig, "app/conf")], &[]);
    let r = resolve_item(&it, &FakePlatform::with_home(home));
    assert!(r.entries.is_empty());
    assert_eq!(
        r.skipped,
        vec![Skipped {
            root: SemanticRoot::UserConfig,
            rel: "app/conf".into(),
            reason: SkipReason::SymlinkInPath {
                ancestor: "app".into()
            },
        }]
    );
}

#[test]
fn overlapping_paths_dedupe_and_unmovable_is_empty() {
    let tmp = tempfile::tempdir().unwrap();
    write(&tmp.path().join(".config/app/a"), 3);
    let mut it = item(
        vec![
            pr(SemanticRoot::UserConfig, "app"),
            pr(SemanticRoot::UserConfig, "app/a"),
        ],
        &[],
    );
    let p = FakePlatform::with_home(tmp.path());
    let r = resolve_item(&it, &p);
    assert_eq!(r.entries.len(), 2);
    assert_eq!(r.total_size, 3);
    it.unmovable = Some("OS-bound".into());
    assert_eq!(resolve_item(&it, &p).entries, vec![]);
}

#[test]
fn root_unavailable_and_bad_glob_are_skips() {
    let tmp = tempfile::tempdir().unwrap();
    let it = item(vec![pr(SemanticRoot::Home, "x")], &["[bad"]);
    let r = resolve_item(&it, &FakePlatform::with_home(tmp.path()));
    assert!(matches!(r.skipped[0].reason, SkipReason::InvalidExclude(_)));
}

#[test]
fn resolves_the_paths_and_excludes_of_the_platforms_os() {
    let tmp = tempfile::tempdir().unwrap();
    let home = tmp.path();
    write(&home.join(".config/demo/a"), 1);
    write(&home.join(".local/share/Demo/b"), 2);
    write(&home.join(".local/share/Demo/b.log"), 3);
    let mut it = item(vec![pr(SemanticRoot::UserConfig, "demo")], &["*.log"]);
    it.os = Some(OsOverrides {
        macos: Some(OsPaths {
            paths: vec![pr(SemanticRoot::UserData, "Demo")],
            exclude: Some(Vec::new()),
        }),
        ..OsOverrides::default()
    });

    let linux = resolve_item(&it, &FakePlatform::with_home(home));
    assert_eq!(
        rels(&linux),
        vec![
            (SemanticRoot::UserConfig, "demo", FileKind::Dir),
            (SemanticRoot::UserConfig, "demo/a", FileKind::File),
        ]
    );
    let mac = resolve_item(&it, &FakePlatform::with_home(home).with_os(OsKind::MacOs));
    assert_eq!(
        rels(&mac),
        vec![
            (SemanticRoot::UserData, "Demo", FileKind::Dir),
            (SemanticRoot::UserData, "Demo/b", FileKind::File),
            (SemanticRoot::UserData, "Demo/b.log", FileKind::File),
        ]
    );
}
