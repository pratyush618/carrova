use super::*;

#[test]
fn rules() {
    let r = parse_rule("default=skip").unwrap();
    assert_eq!(r.target, RuleTarget::Default);
    assert_eq!(r.rule, ConflictRule::Skip);
    assert_eq!(
        parse_rule("git/config=keep-both").unwrap().target,
        RuleTarget::Item("git/config".into())
    );
    assert_eq!(
        parse_rule("raw:user_config/foo=replace").unwrap().target,
        RuleTarget::Item("raw:user_config/foo".into())
    );
    assert_eq!(
        parse_rule("home:.gitconfig=skip").unwrap().target,
        RuleTarget::Path("home:.gitconfig".into())
    );
    assert_eq!(
        parse_rule("git/config:home:.gitconfig=skip")
            .unwrap()
            .target,
        RuleTarget::Path("home:.gitconfig".into())
    );
    assert_eq!(
        parse_rule("raw:home/.foo:home:.foo/a=b=skip")
            .unwrap()
            .target,
        RuleTarget::Path("home:.foo/a=b".into())
    );
    for bad in [
        "git/config",
        "git/config=overwrite",
        "=skip",
        "git/config:nowhere:x=skip",
    ] {
        assert!(parse_rule(bad).is_err(), "{bad}");
    }
}

#[test]
fn remaps() {
    let r = parse_remap("git/config=user_data:old").unwrap();
    assert_eq!(r.item, "git/config");
    assert_eq!(r.remap.root, Some(SemanticRoot::UserData));
    assert_eq!(r.remap.rel_prefix.as_deref(), Some("old"));
    let r = parse_remap("git/config=home").unwrap();
    assert_eq!(r.remap.root, Some(SemanticRoot::Home));
    assert_eq!(r.remap.rel_prefix, None);
    let r = parse_remap("git/config=:backup/x").unwrap();
    assert_eq!(r.remap.root, None);
    assert_eq!(r.remap.rel_prefix.as_deref(), Some("backup/x"));
    for bad in [
        "git/config",
        "=home",
        "git/config=",
        "x=nowhere",
        "x=:",
        "x=home:",
    ] {
        assert!(parse_remap(bad).is_err(), "{bad}");
    }
}

#[test]
fn exclude_paths() {
    let e = parse_exclude_path("ssh/keys=home:.ssh/id_old").unwrap();
    assert_eq!(e.item, "ssh/keys");
    assert_eq!(e.path, "home:.ssh/id_old");
    for bad in ["ssh/keys", "ssh/keys=.ssh/id", "ssh/keys=home:", "=home:x"] {
        assert!(parse_exclude_path(bad).is_err(), "{bad}");
    }
}

#[test]
fn session_ids() {
    assert!(parse_session_id("20261108-101500-a1b2c3").is_ok());
    assert!(parse_session_id("../etc").is_err());
}

#[test]
fn tickets_parse_with_and_without_address() {
    let t = parse_ticket("7-acid-lemon").unwrap();
    assert_eq!(t.addr, None);
    let t = parse_ticket(" 7-acid-lemon@10.0.0.2:47822 ").unwrap();
    assert_eq!(
        t.addr.map(|a| a.to_string()).as_deref(),
        Some("10.0.0.2:47822")
    );
}

#[test]
fn ticket_errors_do_not_repeat_the_input() {
    for bad in ["7-acid-lemonx", "7-acid-lemon@nowhere", "acid-lemon"] {
        let e = parse_ticket(bad).unwrap_err();
        assert!(!e.contains("acid") && !e.contains("lemon"), "{bad}: {e}");
        assert!(e.starts_with("not a pairing code"), "{e}");
    }
}
