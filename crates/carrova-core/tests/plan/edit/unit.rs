use super::*;
use crate::plan::{ConflictRule, PlanOptions, Remap, Retention};

fn offer(key: &str, app: Option<&str>, sensitive: bool) -> Offered {
    Offered {
        key: key.into(),
        app: app.map(Into::into),
        sensitive,
        plugins: Vec::new(),
        safe_pick: false,
    }
}

/// `git/config` (ordinary), `git/token` (sensitive), `ssh/keys`
/// (sensitive, run through plugin `path-rewrite`), `home/notes` (raw).
fn offered() -> Vec<Offered> {
    let mut keys = offer("ssh/keys", Some("ssh"), true);
    keys.plugins = vec![plugin()];
    vec![
        offer("git/config", Some("git"), false),
        offer("git/token", Some("git"), true),
        keys,
        offer("home/notes", None, true),
    ]
}

fn plugin() -> PluginId {
    "path-rewrite".parse().unwrap()
}

fn keys(list: &[&str]) -> Option<Vec<ItemKey>> {
    Some(list.iter().map(|&k| k.into()).collect())
}

fn edited(options: &PlanOptions, edits: Vec<PlanEdit>) -> PlanOptions {
    let mut options = options.clone();
    options.apply_all(edits, &offered()).unwrap();
    options
}

fn bring(item: &str, bring: bool) -> PlanEdit {
    PlanEdit::AnswerSensitive {
        item: item.into(),
        bring,
    }
}

#[test]
fn bringing_a_sensitive_item_confirms_it_and_keeps_the_default_pick() {
    let options = edited(&PlanOptions::default(), vec![bring("git/token", true)]);
    assert_eq!(options.items, None);
    assert_eq!(
        options.confirmed_sensitive,
        vec![ItemKey::from("git/token")]
    );
    let twice = edited(&options, vec![bring("git/token", true)]);
    assert_eq!(twice, options);
}

#[test]
fn leaving_a_sensitive_item_freezes_the_picks_without_it() {
    let start = edited(&PlanOptions::default(), vec![bring("git/token", true)]);
    let options = edited(&start, vec![bring("git/token", false)]);
    assert_eq!(options.items, keys(&["git/config"]));
    assert!(options.confirmed_sensitive.is_empty());
}

#[test]
fn answering_with_an_explicit_list_adds_or_removes_the_item() {
    let start = PlanOptions {
        items: keys(&["git/config"]),
        ..PlanOptions::default()
    };
    let brought = edited(
        &start,
        vec![bring("ssh/keys", true), bring("ssh/keys", true)],
    );
    assert_eq!(brought.items, keys(&["git/config", "ssh/keys"]));
    let left = edited(&brought, vec![bring("ssh/keys", false)]);
    assert_eq!(left.items, keys(&["git/config"]));
}

#[test]
fn skipping_an_app_drops_its_items_and_bringing_it_back_skips_unconfirmed_ones() {
    let start = edited(&PlanOptions::default(), vec![bring("ssh/keys", true)]);
    let skip = |skip| PlanEdit::SkipApp {
        app: "git".into(),
        skip,
    };
    let skipped = edited(&start, vec![skip(true)]);
    assert_eq!(skipped.items, keys(&["ssh/keys"]));
    let back = edited(&skipped, vec![skip(false), skip(false)]);
    assert_eq!(back.items, keys(&["ssh/keys", "git/config"]));
}

#[test]
fn allowing_cross_os_picks_the_item_again_and_withdrawing_keeps_the_pick() {
    let start = PlanOptions {
        items: keys(&[]),
        ..PlanOptions::default()
    };
    let allow = |allow| PlanEdit::AllowCrossOs {
        item: "git/config".into(),
        allow,
    };
    let allowed = edited(&start, vec![allow(true), allow(true)]);
    assert_eq!(allowed.items, keys(&["git/config"]));
    assert_eq!(allowed.allow_cross_os, vec![ItemKey::from("git/config")]);
    let withdrawn = edited(&allowed, vec![allow(false)]);
    assert_eq!(withdrawn.items, keys(&["git/config"]));
    assert!(withdrawn.allow_cross_os.is_empty());
    let by_default = edited(&PlanOptions::default(), vec![allow(true)]);
    assert_eq!(by_default.items, None);
}

#[test]
fn allowing_a_plugin_is_a_set_toggle() {
    let allow = |allow| PlanEdit::AllowPlugin {
        plugin: plugin(),
        allow,
    };
    let allowed = edited(&PlanOptions::default(), vec![allow(true), allow(true)]);
    assert_eq!(allowed.confirmed_plugins, vec![plugin()]);
    assert!(
        edited(&allowed, vec![allow(false)])
            .confirmed_plugins
            .is_empty()
    );
}

#[test]
fn rules_and_remaps_set_and_clear_per_key() {
    let remap = Remap {
        root: None,
        rel_prefix: Some("old".into()),
    };
    let options = edited(
        &PlanOptions::default(),
        vec![
            PlanEdit::ItemRule {
                item: "git/config".into(),
                rule: ConflictRule::Skip,
            },
            PlanEdit::PathRule {
                path: "user_config:git/config".into(),
                rule: Some(ConflictRule::KeepBoth),
            },
            PlanEdit::Remap {
                item: "git/config".into(),
                remap: Some(remap.clone()),
            },
        ],
    );
    assert_eq!(options.item_rule(&"git/config".into()), ConflictRule::Skip);
    assert_eq!(
        options.path_rules.get("user_config:git/config"),
        Some(&ConflictRule::KeepBoth)
    );
    assert_eq!(options.remaps.get("git/config"), Some(&remap));
    let cleared = edited(
        &options,
        vec![
            PlanEdit::PathRule {
                path: "user_config:git/config".into(),
                rule: None,
            },
            PlanEdit::Remap {
                item: "git/config".into(),
                remap: None,
            },
        ],
    );
    assert!(cleared.path_rules.is_empty());
    assert!(cleared.remaps.is_empty());
}

#[test]
fn import_wide_choices_set_their_field_and_an_empty_suffix_keeps_the_current_one() {
    let options = edited(
        &PlanOptions::default(),
        vec![
            PlanEdit::KeepBothSuffix {
                suffix: " mine-{date} ".into(),
            },
            PlanEdit::KeepBothSuffix {
                suffix: "  ".into(),
            },
            PlanEdit::Retention {
                retention: Retention::KeepFor { days: 7 },
            },
            PlanEdit::ImportTrust { allow: true },
            PlanEdit::KeepGroupWrite { keep: true },
            PlanEdit::Override {
                check: PlanOverride::Running,
            },
            PlanEdit::Override {
                check: PlanOverride::Space,
            },
        ],
    );
    assert_eq!(options.keep_both_suffix, "mine-{date}");
    assert_eq!(options.retention, Retention::KeepFor { days: 7 });
    assert!(options.import_trust && options.keep_group_write);
    assert!(options.ignore_requires_closed && options.ignore_free_space);
}

#[test]
fn an_unknown_item_or_plugin_is_an_issue_and_changes_nothing() {
    let mut options = PlanOptions::default();
    let issue = options
        .apply_all(
            vec![PlanEdit::ItemRule {
                item: "nope/x".into(),
                rule: ConflictRule::Skip,
            }],
            &offered(),
        )
        .unwrap_err();
    assert_eq!(issue, PlanEditIssue::UnknownItem("nope/x".into()));
    let other: PluginId = "other".parse().unwrap();
    let issue = options
        .apply_all(
            vec![PlanEdit::AllowPlugin {
                plugin: other.clone(),
                allow: true,
            }],
            &offered(),
        )
        .unwrap_err();
    assert_eq!(issue, PlanEditIssue::UnknownPlugin(other));
    assert_eq!(options, PlanOptions::default());
}

#[test]
fn a_plugin_no_longer_in_use_can_still_be_withdrawn() {
    let other: PluginId = "other".parse().unwrap();
    let start = PlanOptions {
        confirmed_plugins: vec![other.clone()],
        ..PlanOptions::default()
    };
    let options = edited(
        &start,
        vec![PlanEdit::AllowPlugin {
            plugin: other,
            allow: false,
        }],
    );
    assert!(options.confirmed_plugins.is_empty());
}

/// [`offered`] plus two raw picks: `home/todo` (safe) and `home/.bashrc`
/// (sensitive for another reason); `home/notes` is safe too.
fn with_safe_picks() -> Vec<Offered> {
    let mut items = offered();
    for o in &mut items {
        o.safe_pick = o.key == "home/notes";
    }
    let mut todo = offer("home/todo", None, true);
    todo.safe_pick = true;
    items.push(todo);
    items.push(offer("home/.bashrc", None, true));
    items
}

fn accept_safe(options: &PlanOptions) -> PlanOptions {
    let mut options = options.clone();
    options
        .apply(PlanEdit::AcceptSafePicks, &with_safe_picks())
        .unwrap();
    options
}

#[test]
fn accept_safe_picks_sets_the_flag_without_confirming_items_one_by_one() {
    let options = accept_safe(&PlanOptions::default());
    assert_eq!(options.items, None);
    assert!(options.accept_safe_picks);
    assert!(options.confirmed_sensitive.is_empty());
}

#[test]
fn accept_safe_picks_adds_to_explicit_list() {
    let start = PlanOptions {
        items: keys(&["git/config"]),
        ..PlanOptions::default()
    };
    let options = accept_safe(&start);
    assert_eq!(
        options.items,
        keys(&["git/config", "home/notes", "home/todo"])
    );
    assert!(options.confirmed_sensitive.is_empty());
}

#[test]
fn accept_safe_picks_twice_is_idempotent() {
    let start = PlanOptions {
        items: keys(&["home/notes"]),
        ..PlanOptions::default()
    };
    let once = accept_safe(&start);
    assert_eq!(accept_safe(&once), once);
}

fn remap(item: &str, to: Option<&str>) -> PlanEdit {
    PlanEdit::Remap {
        item: item.into(),
        remap: to.map(|p| Remap {
            root: None,
            rel_prefix: Some(p.into()),
        }),
    }
}

#[test]
fn moving_a_confirmed_item_drops_its_confirmation() {
    let start = edited(&PlanOptions::default(), vec![bring("git/token", true)]);
    let moved = edited(&start, vec![remap("git/token", Some("elsewhere"))]);
    assert!(moved.confirmed_sensitive.is_empty());

    let confirmed = edited(&moved, vec![bring("git/token", true)]);
    let back = edited(&confirmed, vec![remap("git/token", None)]);
    assert!(back.confirmed_sensitive.is_empty());
}

#[test]
fn leaving_one_safe_pick_after_accept_all_keeps_the_others() {
    let mut options = accept_safe(&PlanOptions::default());
    options
        .apply(bring("home/todo", false), &with_safe_picks())
        .unwrap();
    assert_eq!(options.items, keys(&["git/config", "home/notes"]));
}

#[test]
fn moving_an_item_to_the_same_place_again_keeps_its_confirmation() {
    let start = edited(
        &PlanOptions::default(),
        vec![remap("git/token", Some("moved")), bring("git/token", true)],
    );
    assert_eq!(
        edited(&start, vec![remap("git/token", Some("moved"))]),
        start
    );
}

#[test]
fn leaving_a_safe_pick_records_the_decline_and_bringing_it_clears_it() {
    let left = edited(&PlanOptions::default(), vec![bring("home/notes", false)]);
    assert_eq!(left.declined, vec![ItemKey::from("home/notes")]);
    let brought = edited(&left, vec![bring("home/notes", true)]);
    assert!(brought.declined.is_empty());
}

#[test]
fn accept_all_skips_a_declined_safe_pick() {
    let mut options = PlanOptions::default();
    options
        .apply_all(
            [bring("home/todo", false), PlanEdit::AcceptSafePicks],
            &with_safe_picks(),
        )
        .unwrap();
    assert!(
        !options
            .items
            .as_ref()
            .unwrap()
            .contains(&"home/todo".into())
    );
    assert!(
        options
            .items
            .as_ref()
            .unwrap()
            .contains(&"home/notes".into())
    );
    assert!(!options.confirms(&"home/todo".into(), true));
}
