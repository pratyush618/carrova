//! Binary/path detection rules against a fake platform.

#[path = "../support/mod.rs"]
mod support;

use std::fs;
use std::path::Path;

use carrova_platform::FileKind;
use carrova_platform::fake::FakePlatform;
use carrova_recipe::{Evidence, SemanticRoot, detect, load_str};

const R: &str = r#"
schema: 1
id: demo
name: Demo
category: dev-tools
detect:
  linux:
    - binary: demo
    - path: { root: user_config, rel: demo }
    - path: { root: home, rel: .demorc }
  macos:
    - binary: demo-mac
items:
  - id: config
    label: Config
    paths: [{ root: user_config, rel: demo }]
"#;

#[test]
fn nothing_present() {
    let tmp = tempfile::tempdir().unwrap();
    let r = load_str(R, Path::new("t")).unwrap();
    let d = detect(&r, &FakePlatform::with_home(tmp.path()));
    assert!(!d.detected);
    assert!(d.evidence.is_empty());
}

#[test]
fn binary_and_paths() {
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join(".config/demo")).unwrap();
    fs::write(tmp.path().join(".demorc"), "x").unwrap();
    let r = load_str(R, Path::new("t")).unwrap();
    let p = FakePlatform::with_home(tmp.path()).binary("demo", "/usr/bin/demo");
    let d = detect(&r, &p);
    assert!(d.detected);
    assert_eq!(d.evidence.len(), 3);
    assert_eq!(
        d.evidence[0],
        Evidence::Binary {
            name: "demo".into(),
            path: "/usr/bin/demo".into()
        }
    );
    assert!(matches!(
        &d.evidence[1],
        Evidence::Path { path, kind: FileKind::Dir } if path.root == SemanticRoot::UserConfig
    ));
    assert!(matches!(
        &d.evidence[2],
        Evidence::Path {
            kind: FileKind::File,
            ..
        }
    ));
}

#[test]
fn dangling_symlink_counts_as_present() {
    let tmp = tempfile::tempdir().unwrap();
    crate::support::symlink("/nonexistent/target", &tmp.path().join(".demorc"));
    let r = load_str(R, Path::new("t")).unwrap();
    let d = detect(&r, &FakePlatform::with_home(tmp.path()));
    assert!(d.detected);
    assert!(matches!(
        &d.evidence[0],
        Evidence::Path {
            kind: FileKind::Symlink,
            ..
        }
    ));
}
