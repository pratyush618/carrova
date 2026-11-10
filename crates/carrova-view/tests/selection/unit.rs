use std::collections::BTreeMap;

use carrova_core::ScanReport;
use carrova_core::scan::{PlatformReport, Totals};
use carrova_core::select::{ItemChoice, Selection};
use carrova_platform::OsKind;

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
