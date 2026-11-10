//! Representative engine results the golden tests render.

use std::collections::BTreeMap;
use std::path::PathBuf;

use carrova_core::Ticket;
use carrova_core::checklist::{Checklist, Skipped};
use carrova_core::plan::{
    Action, Blocker, Handler, ObjectKind, Plan, PlanOptions, PlanWarning, PlannedAction,
    PlannedHandler, SpaceNeed,
};
use carrova_core::scan::{
    AppReport, ItemFlags, ItemPortability, ItemReport, KeyInfo, KeyKind, PlatformReport, RawItem,
    ScanReport, SkippedPath, Snapshot, Totals,
};
use carrova_core::select::ItemChoice;
use carrova_core::session::{Role, SessionId, State};
use carrova_core::store::{SessionRecord, Source};
use carrova_core::{Quarantined, RollbackFailure, RollbackReport};
use carrova_crypto::pairing::CertHash;
use carrova_manifest::{Blake3Hash, Manifest, ManifestItem, SourceInfo};
use carrova_platform::OsKind;
use carrova_platform::SemanticRoot;

pub(super) const ESC: &str = "\x1b[1A\x1b[2K";

pub(super) fn action(rel: &str, kind: ObjectKind, action: Action) -> PlannedAction {
    PlannedAction {
        item: "gnupg/keys".into(),
        entry_index: 0,
        kind,
        root: SemanticRoot::Home,
        rel: rel.into(),
        abs: PathBuf::from("/h").join(rel),
        size: 3,
        mode: (kind != ObjectKind::Symlink).then_some(0o600),
        link_target: None,
        mtime_ns: None,
        action,
    }
}

pub(super) fn manifest_item(recipe: &str, id: &str, label: &str, sensitive: bool) -> ManifestItem {
    let mut item = ManifestItem {
        recipe_id: Some(recipe.into()),
        item_id: id.into(),
        label: label.into(),
        flags: carrova_manifest::ItemFlags::default(),
        entries: Vec::new(),
        handler: None,
        artifacts: Vec::new(),
        source_paths: Vec::new(),
    };
    item.flags.sensitive = sensitive;
    item
}

pub(super) fn sample_plan() -> (Plan, Manifest) {
    let mut link = action(".gnupg/cur", ObjectKind::Symlink, Action::Create);
    link.link_target = Some(format!("keys{ESC}v2"));
    let mut git = action(".gitconfig", ObjectKind::File, Action::Create);
    git.item = "git/config".into();
    let mut actions = vec![
        action(".gnupg/gpg.conf", ObjectKind::File, Action::Create),
        action(".gnupg", ObjectKind::Dir, Action::Replace),
        link,
        action(&format!("x{ESC}"), ObjectKind::File, Action::Replace),
        action(
            ".gnupg/old",
            ObjectKind::File,
            Action::Skip {
                reason: format!("rule{ESC}"),
            },
        ),
        action(
            ".gnupg/a.conf",
            ObjectKind::File,
            Action::KeepBoth {
                rel: ".gnupg/a.conf.carrova-20260101".into(),
            },
        ),
        action(".gnupg/same", ObjectKind::File, Action::Identical),
        git,
    ];
    for i in 0..30 {
        actions.push(action(
            &format!(".gnupg/plugins/big/f{i}"),
            ObjectKind::File,
            Action::Create,
        ));
    }
    let plan = Plan {
        session: SessionId("20260101-000000-abcdef".into()),
        options: PlanOptions::default(),
        actions,
        handlers: vec![PlannedHandler {
            item: "gnupg/keys".into(),
            handler: Handler::GpgKeyring,
            backup: vec![(SemanticRoot::Home, ".gnupg/pubring.kbx".into())],
            import_trust: false,
        }],
        bytes_to_write: 99,
        free_space: None,
        missing_apps: vec![format!("app{ESC}"), "vscode".into()],
        running_apps: vec!["firefox".into()],
        app_names: Default::default(),
        mappings: Vec::new(),
        plugins: Vec::new(),
        blockers: vec![Blocker::RunningApp {
            app: format!("blocked{ESC}"),
            name: format!("blocked{ESC}"),
        }],
        warnings: vec![
            PlanWarning {
                item: Some(format!("i{ESC}").into()),
                message: format!("w{ESC}"),
            },
            PlanWarning {
                item: None,
                message: "global".into(),
            },
        ],
        sensitive: BTreeMap::from([("gnupg/keys".into(), "holds private keys".to_owned())]),
    };
    let manifest = Manifest::new(
        0,
        SourceInfo {
            os: OsKind::Linux,
            carrova_version: "0".into(),
            device_name: Some("ezio-thinkpad".into()),
        },
        vec![
            manifest_item("gnupg", "keys", "GnuPG keys", false),
            manifest_item("git", "config", &format!("Git{ESC}home:.bashrc"), false),
            manifest_item("ssh", "keys", "SSH keys", true),
        ],
    );
    (plan, manifest)
}

pub(super) fn needs() -> Vec<SpaceNeed> {
    vec![
        SpaceNeed {
            state_dir: true,
            roots: vec![SemanticRoot::Home],
            bytes: 99,
            needed: 500,
            available: Some(10_000),
            total: Some(64_000_000_000),
        },
        SpaceNeed {
            state_dir: false,
            roots: vec![SemanticRoot::UserData],
            bytes: 100,
            needed: 5000,
            available: Some(200),
            total: None,
        },
        SpaceNeed {
            state_dir: false,
            roots: vec![SemanticRoot::Cache],
            bytes: 1,
            needed: 9,
            available: None,
            total: None,
        },
    ]
}

pub(super) fn choice(included: bool) -> ItemChoice {
    ItemChoice {
        included,
        suggested: false,
        sensitive: false,
        confirmed_sensitive: false,
        unmovable: None,
        force_unmovable: false,
        excluded_paths: Vec::new(),
        paths: Vec::new(),
    }
}

pub(super) fn item_report(id: &str, label: &str, size: u64, files: u64) -> ItemReport {
    ItemReport {
        id: id.into(),
        label: label.into(),
        size,
        file_count: files,
        flags: ItemFlags {
            sensitive: false,
            requires_closed: false,
            portability: ItemPortability::SameOs,
            default_selected: true,
        },
        sensitive_reason: None,
        paths: Vec::new(),
        unmovable: None,
        missing: Vec::new(),
        skipped: Vec::new(),
        app_running: false,
        keys: Vec::new(),
    }
}

pub(super) fn app(id: &str, name: &str, items: Vec<ItemReport>) -> AppReport {
    AppReport {
        id: id.into(),
        name: name.into(),
        category: "dev-tools".into(),
        detected: true,
        evidence: vec![format!("binary {id}"), "config dir".into()],
        running: false,
        items,
    }
}

pub(super) fn scan_fixture() -> ScanReport {
    let mut sensitive = item_report("keys", "SSH keys", 4096, 3);
    sensitive.flags.sensitive = true;
    sensitive.flags.default_selected = false;
    sensitive.keys = vec![
        KeyInfo {
            path: "home:.ssh/id_ed25519".into(),
            kind: KeyKind::Ed25519,
            fingerprint: "SHA256:uNiVztksCsDhcc0u9e8BujQXVUpKZIDTMczCvj3tD2s".into(),
            comment: Some(format!("me@host{ESC}")),
        },
        KeyInfo {
            path: "home:.ssh/id_rsa".into(),
            kind: KeyKind::Rsa,
            fingerprint: "SHA256:47DEQpj8HBSa+/TImW+5JCeuQeRkm5NMpJWZG3hSuFU".into(),
            comment: None,
        },
    ];
    let mut closed = item_report("profile", "Profile", 5 * 1024 * 1024, 120);
    closed.flags.requires_closed = true;
    closed.app_running = true;
    closed.skipped = vec![SkippedPath {
        path: "home:.mozilla/lock".into(),
        reason: "socket".into(),
    }];
    let mut gone = item_report("history", "History", 0, 0);
    gone.missing = vec!["home:.zsh_history".into()];
    let mut unmovable = item_report("machine-id", "Machine id", 10, 1);
    unmovable.unmovable = Some("bound to this machine".into());
    ScanReport {
        platform: PlatformReport {
            os: OsKind::Linux,
            roots: BTreeMap::from([(SemanticRoot::Home, "/home/me".to_owned())]),
            free_space: Some(3 * 1024 * 1024 * 1024),
            snapshot: Some(Snapshot::Btrfs),
        },
        apps: vec![
            app(
                "git",
                "Git",
                vec![item_report("config", "Git config", 1536, 1)],
            ),
            app("ssh", "OpenSSH", vec![sensitive, unmovable]),
            app("firefox", "Firefox", vec![closed]),
            app("zsh", "Zsh", vec![gone]),
            AppReport {
                detected: false,
                evidence: Vec::new(),
                items: Vec::new(),
                ..app("vscode", "VS Code", Vec::new())
            },
        ],
        unrecognised: vec![
            RawItem {
                root: SemanticRoot::UserConfig,
                rel: "oddtool".into(),
                size: 2048,
                file_count: 2,
                unreadable: 0,
            },
            RawItem {
                root: SemanticRoot::UserData,
                rel: "locked".into(),
                size: 1,
                file_count: 1,
                unreadable: 4,
            },
        ],
        totals: Totals {
            default_selected_size: 1536,
            default_selected_files: 1,
            recognised_size: 9999,
            unrecognised_size: 2049,
            total_size: 12048,
        },
        warnings: Vec::new(),
    }
}

pub(super) fn sample_checklist(rollback_available: bool) -> Checklist {
    Checklist {
        moved: vec!["git/config".into(), format!("x{ESC}").into()],
        skipped: vec![Skipped {
            what: "ssh/keys".into(),
            reason: "not selected".into(),
        }],
        apps_to_install: vec!["vscode (code)".into()],
        rollback_available,
    }
}

pub(super) fn rollback_report() -> RollbackReport {
    RollbackReport {
        restored: (0..3).map(|i| format!("/h/.config/a/f{i}")).collect(),
        removed: vec!["/h/.gitconfig".into()],
        failed: vec![RollbackFailure {
            target: format!("/h/x{ESC}"),
            reason: "permission denied".into(),
        }],
        quarantined: vec![Quarantined {
            target: "/h/.bashrc".into(),
            moved_to: PathBuf::from("/s/rollback-trash/0"),
        }],
    }
}

pub(super) fn record(id: &str, role: Role, state: State, source: Option<Source>) -> SessionRecord {
    SessionRecord {
        id: SessionId(id.into()),
        role,
        state,
        created_at: 1_700_000_000,
        updated_at: 1_700_000_100,
        source,
        approved: true,
        checklist: None,
        error: None,
        bytes_moved: None,
        peer_os: None,
        expires_at: None,
    }
}

pub(super) fn cert() -> CertHash {
    "ab".repeat(32).parse().unwrap()
}

pub(super) fn sessions_fixture() -> Vec<SessionRecord> {
    let mut lan = record(
        "20261108-120000-aaaaaa",
        Role::Import,
        State::Done,
        Some(Source::Lan {
            peer_cert_hash: Some(cert()),
            manifest_fingerprint: Blake3Hash([0; 32]),
            peer_name: Some(format!("studio{ESC}")),
        }),
    );
    lan.checklist = Some(sample_checklist(true));
    lan.peer_os = Some(OsKind::MacOs);
    lan.expires_at = Some(1_700_000_000 + 14 * 86_400);
    let mut failed = record(
        "20261108-110000-bbbbbb",
        Role::Import,
        State::Failed,
        Some(Source::Lan {
            peer_cert_hash: None,
            manifest_fingerprint: Blake3Hash([0; 32]),
            peer_name: None,
        }),
    );
    failed.error = Some(format!("boom{ESC}"));
    failed.approved = false;
    vec![
        lan,
        failed,
        record(
            "20261108-100000-cccccc",
            Role::Export,
            State::RolledBack,
            Some(Source::Archive {
                path: format!("/tmp/a{ESC}.carrova").into(),
                fingerprint: None,
            }),
        ),
        record("20261108-090000-dddddd", Role::Export, State::Scanned, None),
    ]
}

pub(super) fn ticket(s: &str) -> Ticket {
    s.parse().unwrap()
}
