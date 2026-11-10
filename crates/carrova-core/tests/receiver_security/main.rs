//! Receiver must not trust archive-controlled data: sensitivity is decided
//! locally (recipe flags, recipe confinement, built-in dangerous paths),
//! modes are never loosened, and symlink targets never go through an
//! existing link.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::path::{Path, PathBuf};

use carrova_core::ItemKey;
use carrova_core::plan::{Action, Plan, PlanOptions, Remap, plan_on_date};
use carrova_core::session::SessionId;
use carrova_manifest::{
    Artifact, DEFAULT_CHUNK_PARAMS, Entry, ItemFlags, Manifest, ManifestItem, SourceInfo,
    chunk_reader, entry_from_path,
};
use carrova_platform::fake::{FakePlatform, symlink};
use carrova_platform::{OsKind, Platform, SemanticRoot};
use carrova_recipe::Recipe;
use common::mkdir;
use tempfile::TempDir;

const DATE: &str = "20261108";

struct Env {
    _tmp: TempDir,
    sender_home: PathBuf,
    recv_home: PathBuf,
    sender: FakePlatform,
    recv: FakePlatform,
}

fn env() -> Env {
    let tmp = TempDir::new().unwrap();
    let sender_home = tmp.path().join("sender");
    let recv_home = tmp.path().join("recv");
    for h in [&sender_home, &recv_home] {
        std::fs::create_dir_all(h.join(".config")).unwrap();
        std::fs::create_dir_all(h.join("Documents")).unwrap();
    }
    let sender = FakePlatform::with_home(&sender_home);
    let recv = FakePlatform::with_home(&recv_home).with_state_dir(tmp.path().join("state"));
    Env {
        _tmp: tmp,
        sender_home,
        recv_home,
        sender,
        recv,
    }
}

/// Sender file `(root, rel)` with `mode`, captured as a manifest entry.
fn file(e: &Env, root: SemanticRoot, rel: &str, mode: u32) -> Entry {
    let base = e.sender.resolve_root(root).unwrap();
    let path = base.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, b"payload").unwrap();
    e.sender.set_mode(&path, mode).unwrap();
    entry_from_path(&e.sender, root, rel, DEFAULT_CHUNK_PARAMS).unwrap()
}

fn item(recipe: Option<&str>, id: &str, entries: Vec<Entry>) -> ManifestItem {
    ManifestItem {
        recipe_id: recipe.map(str::to_owned),
        item_id: id.into(),
        label: id.into(),
        flags: ItemFlags::default(),
        entries,
        handler: None,
        artifacts: Vec::new(),
        source_paths: Vec::new(),
    }
}

fn manifest(items: Vec<ManifestItem>) -> Manifest {
    let m = Manifest::new(
        0,
        SourceInfo {
            os: OsKind::Linux,
            carrova_version: "test".into(),
            device_name: None,
        },
        items,
    );
    m.validate().unwrap();
    m
}

fn load(yaml: &str, name: &str) -> Recipe {
    carrova_recipe::load_str(yaml, Path::new(&format!("{name}/recipe.yaml"))).unwrap()
}

fn recipes() -> Vec<Recipe> {
    vec![
        load(
            include_str!("../../../../recipes/security/ssh/recipe.yaml"),
            "ssh",
        ),
        load(
            include_str!("../../../../recipes/security/gnupg/recipe.yaml"),
            "gnupg",
        ),
        load(
            include_str!("../../../../recipes/shells/bash/recipe.yaml"),
            "bash",
        ),
        load(
            include_str!("../../../../recipes/shells/zsh/recipe.yaml"),
            "zsh",
        ),
        load(
            r"
schema: 1
id: extras
name: Extras
category: test
detect:
  linux:
    - binary: extras
items:
  - id: completion
    label: Bash completion file
    paths:
      - { root: home, rel: .bash_completion }
  - id: completions
    label: Bash completion scripts
    paths:
      - { root: user_data, rel: bash-completion }
  - id: prezto
    label: Prezto
    paths:
      - { root: home, rel: .zprezto }
  - id: xsession
    label: X session script
    paths:
      - { root: home, rel: .xsession }
",
            "extras",
        ),
        load(
            r"
schema: 1
id: app
name: App
category: test
detect:
  linux:
    - binary: app
items:
  - id: config
    label: Config
    paths:
      - { root: user_config, rel: app }
  - id: autostart
    label: Autostart entry (as the sender declared it)
    paths:
      - { root: home, rel: .config/autostart }
",
            "app",
        ),
    ]
}

fn run(e: &Env, m: &Manifest, o: &PlanOptions) -> Plan {
    plan_on_date(
        &e.recv,
        &recipes(),
        m,
        o,
        &SessionId("20261108-000000-abcdef".into()),
        DATE,
    )
    .unwrap()
}

fn select(keys: &[&str]) -> PlanOptions {
    PlanOptions {
        items: Some(keys.iter().copied().map(ItemKey::from).collect()),
        ..PlanOptions::default()
    }
}

fn confirm(keys: &[&str]) -> PlanOptions {
    PlanOptions {
        confirmed_sensitive: keys.iter().copied().map(ItemKey::from).collect(),
        ..PlanOptions::default()
    }
}

fn has_action(p: &Plan, item: &str) -> bool {
    p.actions.iter().any(|a| a.item == item)
}

fn reason<'p>(p: &'p Plan, item: &str) -> &'p str {
    p.sensitive
        .get(item)
        .unwrap_or_else(|| panic!("{item} not sensitive: {:#?}", p.sensitive))
}

// ------------------------------------------------------------- C1 sensitivity

#[test]
fn crafted_ssh_config_with_authorized_keys_is_not_auto_selected() {
    let e = env();
    let m = manifest(vec![item(
        Some("ssh"),
        "config",
        vec![
            file(&e, SemanticRoot::Home, ".ssh/config", 0o644),
            file(&e, SemanticRoot::Home, ".ssh/authorized_keys", 0o600),
        ],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(!has_action(&p, "ssh/config"), "{:#?}", p.actions);
    let why = reason(&p, "ssh/config");
    assert!(why.contains("outside recipe ssh/config"), "{why}");
    assert!(p.blockers.is_empty());

    // Selected without confirmation → blocker; with it → planned.
    let p = run(&e, &m, &select(&["ssh/config"]));
    assert!(
        p.blockers
            .iter()
            .any(|b| b.to_string().contains("ssh/config"))
    );
    let mut o = select(&["ssh/config"]);
    o.confirmed_sensitive = vec!["ssh/config".into()];
    let p = run(&e, &m, &o);
    assert!(p.blockers.is_empty(), "{:?}", p.blockers);
    assert!(has_action(&p, "ssh/config"));
}

#[test]
fn archive_flag_false_does_not_override_local_recipe_sensitivity() {
    let e = env();
    // Archive claims ssh/keys is not sensitive.
    let m = manifest(vec![item(
        Some("ssh"),
        "keys",
        vec![file(&e, SemanticRoot::Home, ".ssh/id_ed25519", 0o600)],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(!has_action(&p, "ssh/keys"));
    assert!(reason(&p, "ssh/keys").contains("sensitive"));
}

#[test]
fn raw_items_always_need_confirmation() {
    let e = env();
    let m = manifest(vec![item(
        None,
        "raw:documents/notes",
        vec![file(&e, SemanticRoot::Documents, "notes/todo.txt", 0o644)],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(!has_action(&p, "raw:documents/notes"));
    assert!(reason(&p, "raw:documents/notes").contains("unrecognised data"));
    let p = run(&e, &m, &select(&["raw:documents/notes"]));
    assert!(!p.blockers.is_empty());
    let p = run(&e, &m, &confirm(&["raw:documents/notes"]));
    assert!(p.blockers.is_empty());
    assert!(has_action(&p, "raw:documents/notes"));
}

#[test]
fn unknown_recipe_is_treated_like_raw() {
    let e = env();
    let m = manifest(vec![item(
        Some("mystery"),
        "config",
        vec![file(&e, SemanticRoot::UserConfig, "mystery/a", 0o644)],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(!has_action(&p, "mystery/config"));
    assert!(reason(&p, "mystery/config").contains("unrecognised data"));
}

#[test]
fn recipe_confined_non_dangerous_item_stays_auto_selected() {
    let e = env();
    let m = manifest(vec![item(
        Some("app"),
        "config",
        vec![file(
            &e,
            SemanticRoot::UserConfig,
            "app/settings.json",
            0o644,
        )],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(p.sensitive.is_empty(), "{:?}", p.sensitive);
    assert!(has_action(&p, "app/config"));
}

#[test]
fn shell_startup_files_need_confirmation_with_reason() {
    let e = env();
    let m = manifest(vec![item(
        Some("bash"),
        "config",
        vec![file(&e, SemanticRoot::Home, ".bashrc", 0o644)],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(!has_action(&p, "bash/config"));
    let why = reason(&p, "bash/config");
    assert!(why.contains("login") && why.contains(".bashrc"), "{why}");
}

#[test]
fn remap_into_dangerous_path_is_sensitive() {
    let e = env();
    let m = manifest(vec![item(
        Some("app"),
        "config",
        vec![file(
            &e,
            SemanticRoot::UserConfig,
            "app/settings.json",
            0o644,
        )],
    )]);
    let mut o = PlanOptions::default();
    o.remaps.insert(
        "app/config".into(),
        Remap {
            root: Some(SemanticRoot::Home),
            rel_prefix: Some(".ssh".into()),
        },
    );
    let p = run(&e, &m, &o);
    assert!(!has_action(&p, "app/config"));
    assert!(reason(&p, "app/config").contains(".ssh"));

    // Remap via user_config into autostart.
    let mut o = PlanOptions::default();
    o.remaps.insert(
        "app/config".into(),
        Remap {
            root: None,
            rel_prefix: Some("autostart".into()),
        },
    );
    let p = run(&e, &m, &o);
    assert!(reason(&p, "app/config").contains("autostart"));
}

#[test]
fn dangerous_paths_match_across_root_aliases() {
    let e = env();
    // `home:.config/autostart` is `user_config:autostart` on this machine.
    let m = manifest(vec![item(
        Some("app"),
        "autostart",
        vec![file(
            &e,
            SemanticRoot::Home,
            ".config/autostart/evil.desktop",
            0o644,
        )],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(!has_action(&p, "app/autostart"));
    assert!(reason(&p, "app/autostart").contains("autostart"));

    // And `user_config:git/config` ≡ `home:.config/git/config`.
    let m = manifest(vec![item(
        None,
        "raw:home/.config/git",
        vec![file(&e, SemanticRoot::Home, ".config/git/config", 0o644)],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(reason(&p, "raw:home/.config/git").contains("git"));
}

#[test]
fn handler_items_touching_gnupg_are_sensitive() {
    let e = env();
    let fc = chunk_reader(&b"pubkeys"[..], DEFAULT_CHUNK_PARAMS).unwrap();
    let mut it = item(Some("gnupg"), "public-keyring", Vec::new());
    it.handler = Some("gpg-keyring".into());
    it.artifacts = vec![Artifact {
        name: "pubring.asc".into(),
        size: fc.size,
        hash: fc.file_hash,
        chunks: fc.chunks,
    }];
    let m = manifest(vec![it]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(p.handlers.is_empty());
    assert!(reason(&p, "gnupg/public-keyring").contains(".gnupg"));
    let p = run(&e, &m, &confirm(&["gnupg/public-keyring"]));
    assert_eq!(p.handlers.len(), 1);
}

// ------------------------------------------------------------- H2 modes

#[test]
#[cfg(unix)] // Unix mode bits
fn incoming_modes_never_group_or_world_writable() {
    let e = env();
    let m = manifest(vec![item(
        Some("app"),
        "config",
        vec![file(&e, SemanticRoot::UserConfig, "app/run.sh", 0o777)],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    let a = p.actions.iter().find(|a| a.rel == "app/run.sh").unwrap();
    assert_eq!(a.mode, Some(0o755));
    assert_eq!(
        mode_warnings(&p, "app/config"),
        vec!["group-write, world-write dropped on 1 path(s)"]
    );
}

#[test]
#[cfg(unix)] // Unix mode bits
fn existing_directory_mode_is_never_loosened() {
    let e = env();
    mkdir(&e.sender, &e.sender_home, ".ssh", 0o777);
    let dir = entry_from_path(&e.sender, SemanticRoot::Home, ".ssh", DEFAULT_CHUNK_PARAMS).unwrap();
    assert_eq!(dir.mode, Some(0o777));
    mkdir(&e.recv, &e.recv_home, ".ssh", 0o700);
    let m = manifest(vec![item(Some("ssh"), "keys", vec![dir])]);
    let p = run(&e, &m, &confirm(&["ssh/keys"]));
    let a = p.actions.iter().find(|a| a.rel == ".ssh").unwrap();
    assert_eq!(a.mode, Some(0o700));
    assert_eq!(a.action, Action::Identical);

    // Existing 0o755, incoming 0o700 → tightened to 0o700 (set mode).
    e.recv.set_mode(&e.recv_home.join(".ssh"), 0o755).unwrap();
    e.sender
        .set_mode(&e.sender_home.join(".ssh"), 0o700)
        .unwrap();
    let dir = entry_from_path(&e.sender, SemanticRoot::Home, ".ssh", DEFAULT_CHUNK_PARAMS).unwrap();
    let m = manifest(vec![item(Some("ssh"), "keys", vec![dir])]);
    let p = run(&e, &m, &confirm(&["ssh/keys"]));
    let a = p.actions.iter().find(|a| a.rel == ".ssh").unwrap();
    assert_eq!(a.mode, Some(0o700));
    assert_eq!(a.action, Action::Replace);
}

/// Regression: `zsh/oh-my-zsh-custom` (plugin code sourced at every shell
/// start, incl. a plugin's `.git/hooks`) was auto-selected.
#[test]
fn oh_my_zsh_tree_needs_confirmation() {
    let e = env();
    let m = manifest(vec![item(
        Some("zsh"),
        "oh-my-zsh-custom",
        vec![
            file(
                &e,
                SemanticRoot::Home,
                ".oh-my-zsh/custom/plugins/p/p.plugin.zsh",
                0o644,
            ),
            file(
                &e,
                SemanticRoot::Home,
                ".oh-my-zsh/custom/plugins/p/.git/hooks/post-checkout",
                0o755,
            ),
        ],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert!(!has_action(&p, "zsh/oh-my-zsh-custom"), "{:#?}", p.actions);
    let why = reason(&p, "zsh/oh-my-zsh-custom");
    assert!(why.contains("shell start"), "{why}");
    let p = run(&e, &m, &confirm(&["zsh/oh-my-zsh-custom"]));
    assert!(has_action(&p, "zsh/oh-my-zsh-custom"));
}

#[test]
fn shell_code_dirs_need_confirmation() {
    let e = env();
    let m = manifest(vec![
        item(
            Some("extras"),
            "completion",
            vec![file(&e, SemanticRoot::Home, ".bash_completion", 0o644)],
        ),
        item(
            Some("extras"),
            "completions",
            vec![file(
                &e,
                SemanticRoot::UserData,
                "bash-completion/completions/tool",
                0o644,
            )],
        ),
        item(
            Some("extras"),
            "prezto",
            vec![file(&e, SemanticRoot::Home, ".zprezto/init.zsh", 0o644)],
        ),
        item(
            Some("extras"),
            "xsession",
            vec![file(&e, SemanticRoot::Home, ".xsession", 0o755)],
        ),
    ]);
    let p = run(&e, &m, &PlanOptions::default());
    for k in [
        "extras/completion",
        "extras/completions",
        "extras/prezto",
        "extras/xsession",
    ] {
        assert!(!has_action(&p, k), "{k} auto-selected");
        assert!(!reason(&p, k).is_empty());
    }
}

// ------------------------------------------------------------- group-write

#[cfg(unix)]
fn mode_of(p: &Plan, rel: &str) -> Option<u32> {
    p.actions.iter().find(|a| a.rel == rel).unwrap().mode
}

#[cfg(unix)]
fn mode_warnings<'p>(p: &'p Plan, item: &str) -> Vec<&'p str> {
    p.warnings
        .iter()
        .filter(|w| w.item.as_ref().is_some_and(|k| k == item) && w.message.contains("dropped"))
        .map(|w| w.message.as_str())
        .collect()
}

#[test]
#[cfg(unix)] // Unix mode bits
fn group_write_dropped_by_default_kept_with_option() {
    let e = env();
    let m = manifest(vec![item(
        Some("app"),
        "config",
        vec![
            file(&e, SemanticRoot::UserConfig, "app/a", 0o664),
            file(&e, SemanticRoot::UserConfig, "app/b", 0o666),
        ],
    )]);
    let p = run(&e, &m, &PlanOptions::default());
    assert_eq!(mode_of(&p, "app/a"), Some(0o644));
    assert_eq!(mode_of(&p, "app/b"), Some(0o644));
    assert!(
        !p.warnings.iter().any(|w| w.message.contains("override")),
        "{:#?}",
        p.warnings
    );

    let o = PlanOptions {
        keep_group_write: true,
        ..PlanOptions::default()
    };
    let p = run(&e, &m, &o);
    assert_eq!(mode_of(&p, "app/a"), Some(0o664));
    assert_eq!(mode_of(&p, "app/b"), Some(0o664));
    let ov: Vec<_> = p
        .warnings
        .iter()
        .filter(|w| w.message == "override: keeping group-write bits from the archive")
        .collect();
    assert_eq!(ov.len(), 1, "{:#?}", p.warnings);
}

#[test]
#[cfg(unix)] // Unix mode bits
fn group_write_option_never_loosens_existing_dir() {
    let e = env();
    mkdir(&e.sender, &e.sender_home, ".config/app", 0o775);
    let dir = entry_from_path(
        &e.sender,
        SemanticRoot::UserConfig,
        "app",
        DEFAULT_CHUNK_PARAMS,
    )
    .unwrap();
    mkdir(&e.recv, &e.recv_home, ".config/app", 0o755);
    let m = manifest(vec![item(Some("app"), "config", vec![dir])]);
    let o = PlanOptions {
        keep_group_write: true,
        ..PlanOptions::default()
    };
    let p = run(&e, &m, &o);
    assert_eq!(mode_of(&p, "app"), Some(0o755));
}

/// Regression: one "mode 0644 instead of 0664" warning per file (14+ on a
/// umask-002 laptop). Now one per item.
#[test]
#[cfg(unix)] // Unix mode bits
fn mode_warnings_collapse_to_one_per_item() {
    let e = env();
    let m = manifest(vec![
        item(
            Some("app"),
            "config",
            vec![
                file(&e, SemanticRoot::UserConfig, "app/a", 0o664),
                file(&e, SemanticRoot::UserConfig, "app/b", 0o664),
                file(&e, SemanticRoot::UserConfig, "app/c", 0o664),
                file(&e, SemanticRoot::UserConfig, "app/ok", 0o644),
            ],
        ),
        item(
            Some("app"),
            "autostart",
            vec![file(
                &e,
                SemanticRoot::Home,
                ".config/autostart/x.desktop",
                0o777,
            )],
        ),
    ]);
    let p = run(&e, &m, &confirm(&["app/autostart"]));
    let w = mode_warnings(&p, "app/config");
    assert_eq!(
        w,
        vec!["group-write dropped on 3 path(s) (keep with --keep-group-write)"],
        "{:#?}",
        p.warnings
    );
    let w = mode_warnings(&p, "app/autostart");
    assert_eq!(w.len(), 1, "{:#?}", p.warnings);
    assert!(
        w[0] == "group-write, world-write dropped on 1 path(s)",
        "{}",
        w[0]
    );
    // No per-path mode warning remains.
    assert!(
        !p.warnings.iter().any(|w| w.message.contains("instead of")),
        "{:#?}",
        p.warnings
    );
}

// ------------------------------------------------------------- L3 symlinks

#[test]
fn symlink_target_through_existing_link_is_skipped() {
    let e = env();
    // Sender: user_config:app/link -> ../shared/x (lexically inside home).
    mkdir(&e.sender, &e.sender_home, ".config/app", 0o755);
    symlink("../shared/x", e.sender_home.join(".config/app/link")).unwrap();
    let link = entry_from_path(
        &e.sender,
        SemanticRoot::UserConfig,
        "app/link",
        DEFAULT_CHUNK_PARAMS,
    )
    .unwrap();
    let m = manifest(vec![item(Some("app"), "config", vec![link])]);

    // Receiver without `shared`: fine.
    let p = run(&e, &m, &PlanOptions::default());
    assert_eq!(p.actions[0].action, Action::Create);

    // Receiver where `.config/shared` is an existing symlink (to /etc):
    // the target goes through it → skipped.
    symlink("/etc", e.recv_home.join(".config/shared")).unwrap();
    let p = run(&e, &m, &PlanOptions::default());
    assert!(
        matches!(&p.actions[0].action, Action::Skip { reason } if reason.contains("existing link")),
        "{:?}",
        p.actions[0].action
    );
}
