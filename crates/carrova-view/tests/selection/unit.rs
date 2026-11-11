use std::collections::BTreeMap;

use carrova_core::ScanReport;
use carrova_core::scan::{PlatformReport, Totals};
use carrova_core::select::{ItemChoice, Pick, PickFacts, PickKind, Selection};
use carrova_platform::{OsKind, SemanticRoot};

use super::{ItemRoles, SelectionTag, selection_view};

fn empty_scan() -> ScanReport {
    ScanReport {
        platform: PlatformReport {
            os: OsKind::Linux,
            roots: BTreeMap::new(),
            free_space: None,
            snapshot: None,
        },
        apps: Vec::new(),
        unrecognised: Vec::new(),
        totals: Totals::default(),
        warnings: Vec::new(),
    }
}

fn choice(included: bool) -> ItemChoice {
    ItemChoice {
        included,
        suggested: false,
        sensitive: false,
        confirmed_sensitive: false,
        unmovable: None,
        force_unmovable: false,
        excluded_paths: Vec::new(),
        paths: Vec::new(),
        pick: None,
    }
}

#[test]
fn unticked_raw_dirs_are_counted_not_listed() {
    let mut sel = Selection::default();
    sel.items.insert("git/config".into(), choice(false));
    sel.items.insert("raw:home/a".into(), choice(true));
    sel.items.insert("raw:home/b".into(), choice(false));
    let scan = empty_scan();
    let v = selection_view(&sel, &scan);
    let keys: Vec<_> = v.rows.iter().map(|r| r.key).collect();
    assert_eq!(keys, ["git/config", "raw:home/a"]);
    assert_eq!((v.hidden_raw, v.included, v.included_size), (1, 1, 0));
}

#[test]
fn tags_follow_choice_marks_in_order() {
    let mut c = choice(true);
    c.suggested = true;
    c.sensitive = true;
    c.unmovable = Some("bound".into());
    c.force_unmovable = true;
    let mut sel = Selection::default();
    sel.items.insert("ssh/keys".into(), c);
    let scan = empty_scan();
    let v = selection_view(&sel, &scan);
    assert_eq!(
        v.rows[0].tags,
        [
            SelectionTag::Suggested,
            SelectionTag::Sensitive { confirmed: false },
            SelectionTag::Unmovable {
                reason: "bound",
                forced: true
            },
        ]
    );
}

#[test]
fn rows_carry_the_engine_roles_of_their_choice() {
    let mut forced = choice(true);
    forced.unmovable = Some("bound".into());
    forced.force_unmovable = true;
    let mut cache = choice(false);
    cache.paths = vec!["cache:app".into()];
    let mut secret = choice(false);
    secret.sensitive = true;
    let mut sel = Selection::default();
    sel.items.insert("a/forced".into(), forced);
    sel.items.insert("a/cache".into(), cache);
    sel.items.insert("a/secret".into(), secret);
    let scan = empty_scan();
    let roles: Vec<_> = selection_view(&sel, &scan)
        .rows
        .iter()
        .map(|r| (r.key.as_str(), r.roles))
        .collect();
    let role = |ordinary, movable, cache| ItemRoles {
        ordinary,
        movable,
        cache,
    };
    assert_eq!(
        roles,
        [
            ("a/cache", role(true, true, true)),
            ("a/forced", role(false, true, false)),
            ("a/secret", role(false, true, false)),
        ]
    );
}

#[test]
fn an_owned_scan_reply_serialises_as_the_scan_result() {
    let mut sel = Selection::default();
    sel.items.insert("git/config".into(), choice(true));
    let scan = empty_scan();
    let session = carrova_core::SessionId("s1".into());
    let borrowed = crate::ipc::ScanResult::new(session.clone(), &scan, &sel);
    let owned = crate::ipc::ScanReply::new(session, scan.clone(), sel.clone());
    assert_eq!(
        serde_json::to_value(owned).expect("json"),
        serde_json::to_value(borrowed).expect("json")
    );
}

/// A scan whose home is `/h` and config `/h/.config`.
fn scan_with_home() -> ScanReport {
    let mut scan = empty_scan();
    scan.platform.roots = BTreeMap::from([
        (SemanticRoot::Home, "/h".into()),
        (SemanticRoot::UserConfig, "/h/.config".into()),
        (SemanticRoot::Documents, "/elsewhere/Docs".into()),
    ]);
    scan
}

fn picked(sel: &mut Selection, root: SemanticRoot, rel: &str, size: u64, sensitive: bool) {
    let facts = PickFacts {
        kind: PickKind::Dir,
        size,
        files: 1,
    };
    sel.add_pick(Pick {
        root,
        rel: rel.into(),
        facts,
        sensitive,
    });
}

#[test]
fn picks_are_listed_separately_with_home_display() {
    let mut sel = Selection::default();
    sel.items.insert("git/config".into(), choice(true));
    picked(&mut sel, SemanticRoot::Home, "proj/site", 7, false);
    picked(&mut sel, SemanticRoot::UserConfig, "tool", 3, false);
    picked(&mut sel, SemanticRoot::Documents, "cv.pdf", 5, false);
    let scan = scan_with_home();

    let v = selection_view(&sel, &scan);

    let rows: Vec<_> = v.rows.iter().map(|r| r.key.as_str()).collect();
    assert_eq!(rows, ["git/config"]);
    let picks: Vec<_> = v.picks.iter().map(|p| p.display.as_str()).collect();
    assert_eq!(
        picks,
        ["/elsewhere/Docs/cv.pdf", "~/proj/site", "~/.config/tool"]
    );
    assert_eq!((v.included, v.included_size), (4, 15));
}

#[test]
fn unticked_sensitive_pick_stays_visible() {
    let mut sel = Selection::default();
    picked(&mut sel, SemanticRoot::Home, ".ssh", 1, true);
    let scan = scan_with_home();

    let v = selection_view(&sel, &scan);

    let pick = &v.picks[0];
    assert!(!pick.included);
    assert_eq!(pick.tags, [SelectionTag::Sensitive { confirmed: false }]);
    assert_eq!((v.hidden_raw, v.included, v.included_size), (0, 0, 0));
}

#[test]
fn hidden_raw_does_not_count_picks() {
    let mut sel = Selection::default();
    sel.items.insert("raw:home/a".into(), choice(false));
    picked(&mut sel, SemanticRoot::Home, "b", 1, true);
    let scan = scan_with_home();

    let v = selection_view(&sel, &scan);

    assert_eq!(v.hidden_raw, 1);
    assert_eq!(v.picks.len(), 1);
}
