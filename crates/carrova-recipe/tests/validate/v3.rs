//! Schema v3 rules: per-OS overrides, plugin handlers, detect on any OS.

use carrova_platform::OsKind;
use carrova_recipe::{IssueKind, Recipe, validate};

use super::recipe;

const V3: &str = r#"
schema: 3
id: demo
name: Demo
category: dev-tools
detect:
  linux: [{ binary: demo }]
items:
  - id: config
    label: Config
    paths:
      - { root: user_config, rel: demo }
      - { root: home, rel: .demorc }
    os:
      windows:
        paths:
          - { root: user_config, rel: Demo }
          - { root: home, rel: .demorc }
"#;

fn issues(r: &Recipe) -> Vec<(String, IssueKind)> {
    validate(r)
        .into_iter()
        .map(|i| (i.location, i.kind))
        .collect()
}

#[test]
fn aligned_os_override_is_clean() {
    assert_eq!(issues(&recipe(V3)), vec![]);
}

#[test]
fn os_override_with_another_path_count_is_misaligned() {
    let mut r = recipe(V3);
    let windows = r.items[0].os.as_mut().unwrap().windows.as_mut().unwrap();
    windows.paths.pop();
    assert_eq!(
        issues(&r),
        vec![(
            "items[0].os.windows.paths".into(),
            IssueKind::OsPathsMisaligned {
                os: OsKind::Windows,
                base: 2,
                found: 1
            }
        )]
    );
}

#[test]
fn misalignment_message_names_the_os_and_both_counts() {
    let msg = IssueKind::OsPathsMisaligned {
        os: OsKind::MacOs,
        base: 2,
        found: 3,
    }
    .to_string();
    assert!(
        msg.starts_with("macOS override has 3 path(s) but the item has 2"),
        "{msg}"
    );
}

#[test]
fn os_override_excludes_are_checked_like_base_excludes() {
    let mut r = recipe(V3);
    let windows = r.items[0].os.as_mut().unwrap().windows.as_mut().unwrap();
    windows.exclude = Some(vec![String::new(), "[unclosed".into()]);
    let got = issues(&r);
    assert_eq!(
        got[0],
        ("items[0].os.windows.exclude[0]".into(), IssueKind::Empty)
    );
    assert!(matches!(
        &got[1],
        (at, IssueKind::InvalidGlob { glob, .. })
            if at == "items[0].os.windows.exclude[1]" && glob == "[unclosed"
    ));
    assert_eq!(got.len(), 2);
}

#[test]
fn v3_fields_need_schema_3() {
    let mut r = recipe(V3);
    r.schema = 2;
    r.items[0].handler = Some(carrova_recipe::HandlerRef::Plugin(
        "demo-sync".parse().unwrap(),
    ));
    assert_eq!(
        issues(&r),
        vec![
            (
                "items[0].handler".into(),
                IssueKind::RequiresSchema {
                    field: "handler.plugin",
                    min: 3
                }
            ),
            (
                "items[0].os".into(),
                IssueKind::RequiresSchema {
                    field: "os",
                    min: 3
                }
            ),
        ]
    );
}

#[test]
fn v3_recipe_may_target_only_macos_or_windows() {
    let mut r = recipe(V3);
    r.detect.macos = std::mem::take(&mut r.detect.linux);
    assert_eq!(issues(&r), vec![]);
    assert!(r.supports(OsKind::MacOs));
    assert!(!r.supports(OsKind::Linux));
}

#[test]
fn v3_recipe_without_any_detect_rule_is_reported_once() {
    let mut r = recipe(V3);
    r.detect.linux.clear();
    assert_eq!(issues(&r), vec![("detect".into(), IssueKind::NoDetect)]);
}

#[test]
fn v2_recipe_still_requires_linux_detect() {
    let mut r = recipe(V3);
    r.schema = 2;
    r.items[0].os = None;
    r.detect.macos = std::mem::take(&mut r.detect.linux);
    assert_eq!(
        issues(&r),
        vec![("detect.linux".into(), IssueKind::NoLinuxDetect)]
    );
}

#[test]
fn sensitive_path_captured_only_on_one_os_is_reported_at_its_override() {
    let text = V3.replace(
        "items:\n",
        "items:\n  - id: token\n    label: Token\n    paths: [{ root: user_config, rel: Demo/token }]\n    flags: { sensitive: true }\n",
    );
    let r = recipe(&text);
    assert_eq!(
        issues(&r),
        vec![(
            "items[1].os.windows.paths[0]".into(),
            IssueKind::SensitiveCapturedByNonSensitive {
                path: r.items[0].paths[0].clone(),
                sensitive_item: "token".into(),
                by_item: "config".into(),
            }
        )]
    );
}
