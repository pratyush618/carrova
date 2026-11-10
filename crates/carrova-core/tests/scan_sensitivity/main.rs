//! The sender's scan says why an item needs the user's OK — the same
//! text the receiver's plan shows for a recipe-flagged item.
#![allow(clippy::unwrap_used)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use carrova_core::plan::recipe_sensitive;
use carrova_core::{ScanOptions, scan};
use carrova_platform::fake::FakePlatform;
use tempfile::TempDir;

const RECIPE: &str = r"
schema: 2
id: app
name: The App
category: test
detect:
  linux:
    - binary: app
items:
  - id: config
    label: Config
    paths:
      - { root: user_config, rel: app }
  - id: keys
    label: Keys
    paths:
      - { root: home, rel: .ssh }
    flags:
      sensitive: true
";

#[test]
fn sensitive_scan_items_carry_the_recipe_reason_and_others_none() {
    let tmp = TempDir::new().unwrap();
    let home = tmp.path().join("home");
    std::fs::create_dir_all(home.join(".config/app")).unwrap();
    std::fs::create_dir_all(home.join(".ssh")).unwrap();
    std::fs::write(home.join(".ssh/id"), b"key").unwrap();
    std::fs::write(home.join(".config/app/c"), b"conf").unwrap();
    let platform = FakePlatform::with_home(&home).binary("app", "/usr/bin/app");

    let report = scan(&common::recipes(RECIPE), &platform, ScanOptions::default()).unwrap();

    let items = &report.apps[0].items;
    let reason = |id: &str| {
        items
            .iter()
            .find(|i| i.id == id)
            .unwrap()
            .sensitive_reason
            .clone()
    };
    assert_eq!(reason("keys"), Some(recipe_sensitive("app", "keys")));
    assert_eq!(reason("config"), None);
}
