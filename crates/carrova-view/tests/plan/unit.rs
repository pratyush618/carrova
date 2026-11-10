use std::collections::BTreeMap;
use std::path::PathBuf;

use carrova_core::plan::{Action, ObjectKind, Plan, PlanOptions, PlannedAction};
use carrova_core::session::SessionId;
use carrova_platform::SemanticRoot;

use carrova_core::plan::SpaceNeed;

use super::{DirGroup, FsSpace, Line, plan_view, space_view};
use crate::Detail;

fn action(item: &str, rel: &str, kind: ObjectKind, action: Action) -> PlannedAction {
    PlannedAction {
        item: item.into(),
        entry_index: 0,
        kind,
        root: SemanticRoot::Home,
        rel: rel.into(),
        abs: PathBuf::from("/h").join(rel),
        size: if kind == ObjectKind::File { 10 } else { 0 },
        mode: (kind != ObjectKind::Symlink).then_some(0o644),
        link_target: (kind == ObjectKind::Symlink).then(|| "t".to_owned()),
        mtime_ns: None,
        action,
    }
}

fn plan(actions: Vec<PlannedAction>) -> Plan {
    Plan {
        session: SessionId("20260101-000000-abcdef".into()),
        options: PlanOptions::default(),
        actions,
        handlers: Vec::new(),
        bytes_to_write: 0,
        free_space: None,
        missing_apps: Vec::new(),
        running_apps: Vec::new(),
        app_names: Default::default(),
        mappings: Vec::new(),
        plugins: Vec::new(),
        blockers: Vec::new(),
        warnings: Vec::new(),
        sensitive: BTreeMap::new(),
    }
}

/// An oh-my-zsh custom dir: one plugin with 20 files, a top-level file,
/// a symlink, one replace and one skip.
fn omz() -> Plan {
    const ITEM: &str = "zsh/oh-my-zsh-custom";
    let mut v = vec![
        action(ITEM, ".oh-my-zsh/custom", ObjectKind::Dir, Action::Create),
        action(
            ITEM,
            ".oh-my-zsh/custom/plugins",
            ObjectKind::Dir,
            Action::Create,
        ),
        action(
            ITEM,
            ".oh-my-zsh/custom/plugins/auto",
            ObjectKind::Dir,
            Action::Create,
        ),
    ];
    for i in 0..20 {
        v.push(action(
            ITEM,
            &format!(".oh-my-zsh/custom/plugins/auto/f{i}"),
            ObjectKind::File,
            Action::Create,
        ));
    }
    v.push(action(
        ITEM,
        ".oh-my-zsh/custom/aliases.zsh",
        ObjectKind::File,
        Action::Create,
    ));
    v.push(action(
        ITEM,
        ".oh-my-zsh/custom/plugins/auto/link",
        ObjectKind::Symlink,
        Action::Create,
    ));
    v.push(action(
        ITEM,
        ".oh-my-zsh/custom/plugins/auto/f0.bak",
        ObjectKind::File,
        Action::Replace,
    ));
    v.push(action(
        ITEM,
        ".oh-my-zsh/custom/old",
        ObjectKind::File,
        Action::Skip {
            reason: "rule".into(),
        },
    ));
    v.push(action(
        ITEM,
        ".oh-my-zsh/custom/same",
        ObjectKind::File,
        Action::Identical,
    ));
    plan(v)
}

fn rels<'a>(lines: &[Line<'a>]) -> Vec<&'a str> {
    lines
        .iter()
        .filter_map(|l| match l {
            Line::Action(a) => Some(a.rel.as_str()),
            Line::Group(_) => None,
        })
        .collect()
}

#[test]
fn summary_collapses_dir_with_many_creates() {
    let p = omz();
    let view = plan_view(&p, Detail::Summary);
    assert!(view.collapsed);
    let item = &view.items[0];
    assert_eq!(item.key, "zsh/oh-my-zsh-custom");
    let groups: Vec<_> = item
        .lines
        .iter()
        .filter_map(|l| match l {
            Line::Group(g) => Some(g),
            Line::Action(_) => None,
        })
        .collect();
    assert_eq!(
        groups,
        [&DirGroup {
            root: SemanticRoot::Home,
            rel: ".oh-my-zsh/custom/plugins".into(),
            files: 20,
            bytes: 200,
        }]
    );
    // Base dir and top-level file as-is; attention lines always listed.
    assert_eq!(
        rels(&item.lines),
        [
            ".oh-my-zsh/custom",
            ".oh-my-zsh/custom/aliases.zsh",
            ".oh-my-zsh/custom/plugins/auto/link",
            ".oh-my-zsh/custom/plugins/auto/f0.bak",
            ".oh-my-zsh/custom/old",
        ]
    );
}

#[test]
fn counts_every_action_and_bytes_written() {
    let p = omz();
    let c = &plan_view(&p, Detail::Summary).items[0].counts;
    let n = c.actions;
    assert_eq!(
        (n.create, n.replace, n.keep_both, n.skip, n.identical),
        (25, 1, 0, 1, 1)
    );
    // 21 created files + 1 replaced file, 10 bytes each.
    assert_eq!(c.bytes, 220);
}

#[test]
fn full_detail_lists_every_non_identical_action() {
    let p = omz();
    let view = plan_view(&p, Detail::Full);
    assert!(!view.collapsed);
    let lines = &view.items[0].lines;
    assert_eq!(lines.len(), p.actions.len() - 1);
    assert!(rels(lines).contains(&".oh-my-zsh/custom/plugins/auto/f7"));
}

#[test]
fn small_dirs_and_single_file_items_not_collapsed() {
    let p = plan(vec![
        action("git/config", ".gitconfig", ObjectKind::File, Action::Create),
        action("a/b", ".config/a", ObjectKind::Dir, Action::Create),
        action("a/b", ".config/a/x", ObjectKind::File, Action::Create),
        action("a/b", ".local/share/a/y", ObjectKind::File, Action::Create),
    ]);
    let view = plan_view(&p, Detail::Summary);
    assert!(!view.collapsed);
    assert_eq!(rels(&view.items[0].lines), [".gitconfig"]);
    assert_eq!(
        rels(&view.items[1].lines),
        [".config/a", ".config/a/x", ".local/share/a/y"]
    );
}

#[test]
fn space_view_lists_each_filesystem_and_shortfall() {
    let mut p = plan(Vec::new());
    p.bytes_to_write = 100;
    let needs = [
        SpaceNeed {
            state_dir: true,
            roots: vec![SemanticRoot::Home],
            bytes: 100,
            needed: 500,
            available: Some(1000),
        },
        SpaceNeed {
            state_dir: false,
            roots: vec![SemanticRoot::UserData],
            bytes: 100,
            needed: 500,
            available: Some(200),
        },
        SpaceNeed {
            state_dir: false,
            roots: vec![SemanticRoot::Cache],
            bytes: 1,
            needed: 9,
            available: None,
        },
    ];
    let v = space_view(&p, &needs);
    assert_eq!(v.writes, 100);
    assert_eq!(
        v.filesystems,
        [
            FsSpace {
                what: "the state dir and home".into(),
                needed: 500,
                available: Some(1000),
                short_by: None,
            },
            FsSpace {
                what: "user_data".into(),
                needed: 500,
                available: Some(200),
                short_by: Some(300),
            },
            FsSpace {
                what: "cache".into(),
                needed: 9,
                available: None,
                short_by: None,
            },
        ]
    );
}

#[test]
fn an_action_line_keeps_both_its_line_tag_and_its_object_kind() {
    let a = action("git/config", ".gitconfig", ObjectKind::File, Action::Create);
    let json = serde_json::to_value(Line::Action(&a)).unwrap_or_default();
    assert_eq!(json["line"], "action");
    assert_eq!(json["kind"], "file");
}
