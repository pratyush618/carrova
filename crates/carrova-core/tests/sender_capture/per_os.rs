//! Capture on a non-Linux source: the item's paths for that OS are walked
//! and recorded as slot-aligned `source_paths`, with the typed source OS.

use carrova_manifest::SourcePath;

use super::*;

const EDITOR: &str = r"
schema: 3
id: editor
name: Editor
category: dev
detect:
  linux: [{ binary: editor }]
  macos: [{ binary: editor }]
items:
  - id: settings
    label: Settings
    paths:
      - { root: user_config, rel: Editor/User }
      - { root: home, rel: .editor }
    os:
      macos:
        paths:
          - { root: user_data, rel: Editor/User }
          - { root: home, rel: .editor }
";

fn source_paths(m: &Manifest) -> Vec<String> {
    m.items[0]
        .source_paths
        .iter()
        .map(|SourcePath { root, rel }| format!("{root}:{rel}"))
        .collect()
}

fn capture_on(os: OsKind) -> Manifest {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path();
    write(&home.join(".config/Editor/User/linux.json"), b"{}");
    write(&home.join(".local/share/Editor/User/mac.json"), b"{}");
    write(&home.join(".editor/ext"), b"e");
    let platform = FakePlatform::with_home(home)
        .with_os(os)
        .binary("editor", "/bin/editor");
    let recipes = vec![load_str(EDITOR, Path::new("editor.yaml")).unwrap()];
    let report = scan(&recipes, &platform, ScanOptions::default()).unwrap();
    let mut sel = Selection::from_scan(&report, Preset::None);
    sel.set_included("editor/settings", true).unwrap();
    let cap = capture(&platform, &recipes, &sel, &NullSink, &Cancel::default()).unwrap();
    assert!(cap.failed.is_empty(), "{:?}", cap.failed);
    cap.manifest
}

#[test]
fn macos_capture_records_the_macos_slots_and_their_files() {
    let m = capture_on(OsKind::MacOs);
    assert_eq!(m.source.os, OsKind::MacOs);
    assert_eq!(source_paths(&m), ["user_data:Editor/User", "home:.editor"]);
    let files = paths(&m);
    assert!(
        files.contains(&"user_data:Editor/User/mac.json".to_owned()),
        "{files:?}"
    );
    assert!(!files.iter().any(|f| f.contains("linux.json")), "{files:?}");
}

#[test]
fn linux_capture_records_the_base_slots() {
    let m = capture_on(OsKind::Linux);
    assert_eq!(m.source.os, OsKind::Linux);
    assert_eq!(
        source_paths(&m),
        ["user_config:Editor/User", "home:.editor"]
    );
    assert!(paths(&m).contains(&"user_config:Editor/User/linux.json".to_owned()));
}
