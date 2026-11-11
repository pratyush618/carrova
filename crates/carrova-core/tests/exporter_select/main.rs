//! A rejected selection edit leaves the exporter unable to export or send
//! until a usable selection is chosen: the engine never captures a
//! selection the user did not end up with.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::sync::Arc;

use carrova_core::{
    CoreError, Engine, Exporter, ItemChoice, ItemKey, KdfParams, Passphrase, Preset, ScanOptions,
    SelectionEdit, State, VecSink, WriteOptions,
};
use carrova_platform::SemanticRoot;
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
  - id: key
    label: Key
    paths:
      - { root: home, rel: .app-key }
    flags: { sensitive: true }
";

fn exporter(tmp: &TempDir) -> Exporter {
    let home = tmp.path().join("home");
    let platform = Arc::new(
        FakePlatform::with_home(&home)
            .with_state_dir(tmp.path().join("state/carrova"))
            .binary("app", "/usr/bin/app"),
    );
    common::put(&*platform, &home, ".config/app/f", b"x", 0o644);
    common::put(&*platform, &home, ".app-key", b"secret", 0o600);
    let engine = Engine::new(
        platform,
        common::recipes(RECIPE),
        Arc::new(VecSink::default()),
    );
    let scan = ScanOptions {
        include_undetected: false,
        unrecognised: false,
    };
    Exporter::start(engine, scan).unwrap()
}

fn write_options() -> WriteOptions {
    WriteOptions {
        kdf: KdfParams::FLOOR,
    }
}

#[test]
fn a_rejected_selection_refuses_export_and_send_until_a_usable_one() {
    let tmp = TempDir::new().unwrap();
    let mut exp = exporter(&tmp);
    let usable = exp.suggest(Preset::Suggested);
    exp.select(usable.clone()).unwrap();
    assert_eq!(exp.state(), State::Selected);

    let mut unconfirmed = usable.clone();
    unconfirmed
        .apply(SelectionEdit::Item {
            item: "app/key".into(),
            included: true,
        })
        .unwrap();
    let err = exp.select(unconfirmed).unwrap_err();
    assert!(matches!(err, CoreError::Selection(_)), "{err}");
    assert!(exp.selection().is_none(), "the earlier selection is gone");

    let archive = tmp.path().join("out.carrova");
    let pass = Passphrase::new("correct horse battery staple".into());
    let err = exp.export(&archive, &pass, write_options()).unwrap_err();
    assert!(matches!(err, CoreError::Selection(_)), "{err}");
    assert!(!archive.exists());
    let err = exp.prepare_send().unwrap_err();
    assert!(matches!(err, CoreError::Selection(_)), "{err}");
    assert_eq!(exp.state(), State::Selected, "nothing was started");

    exp.select(usable).unwrap();
    assert!(exp.selection().is_some());
    exp.export(&archive, &pass, write_options()).unwrap();
    assert_eq!(exp.state(), State::Done);
}

#[test]
fn a_raw_pick_in_a_secret_location_is_sensitive_whatever_the_front_end_says() {
    let tmp = TempDir::new().unwrap();
    let mut exp = exporter(&tmp);
    let key = ItemKey::raw(carrova_core::RootedPath::new(SemanticRoot::Home, ".aws"));
    let mut selection = exp.suggest(Preset::Suggested);
    let mut pick = ItemChoice {
        included: true,
        sensitive: false,
        paths: vec!["home:.aws".into()],
        ..ItemChoice::default()
    };
    selection.items.insert(key.clone(), pick.clone());
    let err = exp.select(selection.clone()).unwrap_err();
    assert!(err.to_string().contains("sensitive"), "{err}");

    pick.confirmed_sensitive = true;
    selection.items.insert(key.clone(), pick);
    exp.select(selection).unwrap();
    assert!(exp.selection().unwrap().items[&key].sensitive);
}
