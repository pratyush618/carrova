use carrova_core::plan::PlanOptions;
use clap::Parser;

use super::{ImportArgs, PlanArgs};

/// `carrova import` arguments on their own.
#[derive(Debug, Parser)]
struct Import {
    #[command(flatten)]
    args: ImportArgs,
}

fn parse(argv: &[&str]) -> PlanArgs {
    let parsed = Import::try_parse_from(
        ["import", "a.carrova"]
            .into_iter()
            .chain(argv.iter().copied()),
    );
    parsed
        .map(|i| i.args.plan)
        .unwrap_or_else(|e| panic!("{e}"))
}

/// The options `argv` gives a new session.
fn options(argv: &[&str]) -> PlanOptions {
    parse(argv).options_over(PlanOptions::default())
}

/// Options a session was approved with: every flag set.
const APPROVED: &[&str] = &[
    "--item",
    "app/config",
    "--confirm-sensitive",
    "user_data/big",
    "--allow-cross-os",
    "app/state",
    "--rule",
    "default=keep-both",
    "--rule",
    "app/config=skip",
    "--remap",
    "app/config=:moved",
    "--ignore-running",
    "--ignore-free-space",
    "--keep-both-suffix",
    "old",
    "--keep-group-write",
    "--import-trust",
    "--discard-rollback",
];

#[test]
fn keep_group_write_flag_sets_plan_option() {
    assert!(!options(&[]).keep_group_write);
    assert!(options(&["--keep-group-write"]).keep_group_write);
}

#[test]
fn without_flags_the_resumed_sessions_options_are_kept() {
    let approved = options(APPROVED);
    assert_ne!(approved, PlanOptions::default());
    assert_eq!(parse(&["--yes"]).options_over(approved.clone()), approved);
}

#[test]
fn a_passed_flag_changes_only_its_own_option() {
    let approved = options(APPROVED);
    let resumed = parse(&["--rule", "default=replace"]).options_over(approved.clone());
    assert_eq!(
        resumed,
        PlanOptions {
            default_rule: PlanOptions::default().default_rule,
            ..approved
        }
    );
}

#[test]
fn a_passed_list_replaces_the_resumed_one() {
    let approved = options(APPROVED);
    let resumed = parse(&["--confirm-sensitive", "ssh/keys"]).options_over(approved);
    assert_eq!(
        resumed.confirmed_sensitive,
        options(&["--confirm-sensitive", "ssh/keys"]).confirmed_sensitive
    );
    assert_eq!(resumed.items, options(APPROVED).items);
}

#[test]
fn a_passed_rule_overrides_that_target_and_keeps_the_others() {
    let approved = options(APPROVED);
    let resumed = parse(&["--rule", "other/item=replace"]).options_over(approved.clone());
    assert_eq!(resumed.item_rules.len(), approved.item_rules.len() + 1);
    assert!(
        approved
            .item_rules
            .iter()
            .all(|(k, r)| resumed.item_rules.get(k) == Some(r))
    );
}

#[test]
fn allow_cross_os_flag_lists_the_overridden_items() {
    assert!(options(&[]).allow_cross_os.is_empty());
    assert_eq!(
        options(&["--allow-cross-os", "a/x", "--allow-cross-os", "b/y"]).allow_cross_os,
        ["a/x", "b/y"]
    );
}

#[test]
fn allow_plugin_flag_lists_the_allowed_plugins() {
    assert!(options(&[]).confirmed_plugins.is_empty());
    let allowed = options(&["--allow-plugin", "path-rewrite", "--allow-plugin", "other"]);
    let ids: Vec<&str> = allowed
        .confirmed_plugins
        .iter()
        .map(carrova_recipe::PluginId::as_str)
        .collect();
    assert_eq!(ids, ["path-rewrite", "other"]);
}

#[test]
fn keep_undo_for_sets_a_retention_of_that_many_days() {
    use carrova_core::plan::Retention;
    assert_eq!(options(&[]).retention, Retention::default());
    assert_eq!(
        options(&["--keep-undo-for", "14"]).retention,
        Retention::KeepFor { days: 14 }
    );
    let parse_err = |argv: &[&str]| {
        Import::try_parse_from(
            ["import", "a.carrova"]
                .into_iter()
                .chain(argv.iter().copied()),
        )
        .is_err()
    };
    assert!(parse_err(&["--keep-undo-for", "0"]));
    assert!(parse_err(&["--keep-undo-for", "7", "--discard-rollback"]));
}
