//! Schema v1/v2 cases: (name, yaml, expected valid), checked against both
//! the JSON Schema and the Rust loader + validator.

pub const BASE: &str = r#"
schema: 1
id: git
name: Git
category: dev-tools
description: Distributed VCS
detect:
  linux:
    - binary: git
    - path: { root: home, rel: .gitconfig }
items:
  - id: config
    label: Global git config
    paths:
      - { root: home, rel: .gitconfig }
      - { root: user_config, rel: git }
    exclude: ["**/credentials"]
    flags:
      portability: same_os
      sensitive: false
      requires_closed: false
      default: true
"#;

pub fn with_item(item: &str) -> String {
    format!("{BASE}{item}")
}

/// (name, yaml, expected valid)
pub fn cases() -> Vec<(&'static str, String, bool)> {
    let r = |from: &str, to: &str| BASE.replacen(from, to, 1);
    vec![
        ("base", BASE.to_owned(), true),
        (
            "no description",
            r("description: Distributed VCS\n", ""),
            true,
        ),
        (
            "flags omitted",
            r(
                "    flags:\n      portability: same_os\n      sensitive: false\n      requires_closed: false\n      default: true\n",
                "",
            ),
            true,
        ),
        ("cross_os", r("same_os", "cross_os"), true),
        (
            "macos detect",
            r("items:", "  macos: [{ binary: git }]\nitems:"),
            true,
        ),
        ("dot-slash rel", r("rel: git }", "rel: ./git/ }"), true),
        (
            "sensitive default omitted",
            with_item(
                "  - id: creds\n    label: Creds\n    paths: [{ root: home, rel: .git-credentials }]\n    flags: { sensitive: true }\n",
            ),
            true,
        ),
        (
            "sensitive default false",
            with_item(
                "  - id: creds\n    label: Creds\n    paths: [{ root: home, rel: .git-credentials }]\n    flags: { sensitive: true, default: false }\n",
            ),
            true,
        ),
        (
            "unmovable no paths",
            with_item("  - id: kr\n    label: Keyring\n    unmovable: OS-bound\n"),
            true,
        ),
        (
            "unmovable default false",
            with_item(
                "  - id: kr\n    label: Keyring\n    unmovable: OS-bound\n    flags: { default: false }\n",
            ),
            true,
        ),
        // ---- invalid ----
        ("schema 4", r("schema: 1", "schema: 4"), false),
        ("schema 0", r("schema: 1", "schema: 0"), false),
        ("schema missing", r("schema: 1\n", ""), false),
        (
            "unknown top field",
            r("name: Git", "name: Git\nhomepage: x"),
            false,
        ),
        ("bad id", r("id: git", "id: Git_Tool"), false),
        (
            "bad category",
            r("category: dev-tools", "category: dev--tools"),
            false,
        ),
        ("empty name", r("name: Git", "name: ''"), false),
        (
            "no linux detect",
            r(
                "  linux:\n    - binary: git\n    - path: { root: home, rel: .gitconfig }\n",
                "  macos: [{ binary: git }]\n",
            ),
            false,
        ),
        (
            "empty linux detect",
            r(
                "  linux:\n    - binary: git\n    - path: { root: home, rel: .gitconfig }\n",
                "  linux: []\n",
            ),
            false,
        ),
        (
            "unknown detect os",
            r("items:", "  bsd: [{ binary: git }]\nitems:"),
            false,
        ),
        (
            "detect both keys",
            r(
                "- binary: git",
                "- { binary: git, path: { root: home, rel: x } }",
            ),
            false,
        ),
        ("detect empty rule", r("- binary: git", "- {}"), false),
        (
            "binary with slash",
            r("- binary: git", "- binary: /usr/bin/git"),
            false,
        ),
        ("bad root", r("root: user_config", "root: etc"), false),
        ("rel parent", r("rel: git }", "rel: ../git }"), false),
        (
            "rel inner parent",
            r("rel: git }", "rel: a/../git }"),
            false,
        ),
        ("rel absolute", r("rel: git }", "rel: /etc/git }"), false),
        ("rel tilde", r("rel: git }", "rel: ~/git }"), false),
        ("rel backslash", r("rel: git }", "rel: 'a\\b' }"), false),
        ("rel drive", r("rel: git }", "rel: 'C:/x' }"), false),
        ("rel empty", r("rel: git }", "rel: '' }"), false),
        ("rel dot", r("rel: git }", "rel: ./ }"), false),
        (
            "unknown path key",
            r("rel: git }", "rel: git, mode: 1 }"),
            false,
        ),
        (
            "no items",
            r(&BASE[BASE.find("items:").unwrap_or(0)..], "items: []\n"),
            false,
        ),
        ("bad item id", r("- id: config", "- id: Config"), false),
        (
            "empty label",
            r("label: Global git config", "label: ''"),
            false,
        ),
        (
            "unknown item field",
            r(
                "label: Global git config",
                "label: G\n    export: git config -l",
            ),
            false,
        ),
        (
            "unknown flag",
            r("sensitive: false", "secret: false"),
            false,
        ),
        ("bad portability", r("same_os", "any_os"), false),
        (
            "non-bool flag",
            r("sensitive: false", "sensitive: maybe"),
            false,
        ),
        ("empty exclude", r("[\"**/credentials\"]", "['']"), false),
        (
            "paths missing",
            with_item("  - id: x\n    label: X\n"),
            false,
        ),
        (
            "paths empty",
            with_item("  - id: x\n    label: X\n    paths: []\n"),
            false,
        ),
        (
            "sensitive default true",
            with_item(
                "  - id: creds\n    label: Creds\n    paths: [{ root: home, rel: .git-credentials }]\n    flags: { sensitive: true, default: true }\n",
            ),
            false,
        ),
        (
            "unmovable default true",
            with_item(
                "  - id: kr\n    label: Keyring\n    unmovable: OS-bound\n    flags: { default: true }\n",
            ),
            false,
        ),
        (
            "unmovable empty reason",
            with_item("  - id: kr\n    label: Keyring\n    unmovable: ''\n"),
            false,
        ),
        // v2-only fields are rejected in v1.
        (
            "v1 processes",
            r("items:", "processes: [git-agent]\nitems:"),
            false,
        ),
        (
            "v1 handler",
            r(
                "label: Global git config",
                "label: Global git config\n    handler: gpg-keyring",
            ),
            false,
        ),
    ]
}

/// v2: every v1 case except those about the version itself, with
/// `schema: 2`, plus the v2-only fields.
pub fn cases_v2() -> Vec<(String, String, bool)> {
    let v2 = BASE.replacen("schema: 1", "schema: 2", 1);
    let r = |from: &str, to: &str| v2.replacen(from, to, 1);
    let handler = |extra: &str| {
        r(
            "label: Global git config",
            &format!("label: Global git config\n    handler: gpg-keyring{extra}"),
        )
    };
    let mut out: Vec<(String, String, bool)> = cases()
        .into_iter()
        .filter(|(name, _, _)| !name.starts_with("schema ") && !name.starts_with("v1 "))
        .map(|(name, yaml, want)| {
            (
                format!("v2 {name}"),
                yaml.replacen("schema: 1", "schema: 2", 1),
                want,
            )
        })
        .collect();
    let extra = [
        ("schema 2", v2.clone(), true),
        (
            "processes",
            r("items:", "processes: [gpg-agent, scdaemon]\nitems:"),
            true,
        ),
        ("processes empty list", r("items:", "processes: []\nitems:"), true),
        ("handler", handler(""), true),
        (
            "handler sensitive",
            with_item(
                "  - id: keys\n    label: Keys\n    handler: gpg-keyring\n    paths: [{ root: home, rel: .gnupg/pubring.kbx }]\n    flags: { sensitive: true }\n",
            )
            .replacen("schema: 1", "schema: 2", 1),
            true,
        ),
        (
            "process with slash",
            r("items:", "processes: [/usr/bin/gpg-agent]\nitems:"),
            false,
        ),
        ("process empty", r("items:", "processes: ['']\nitems:"), false),
        (
            "process not a list",
            r("items:", "processes: gpg-agent\nitems:"),
            false,
        ),
        ("unknown handler", r("label: Global git config", "label: G\n    handler: gpg-export"), false),
        (
            "handler empty",
            r("label: Global git config", "label: G\n    handler: ''"),
            false,
        ),
        (
            "handler without paths",
            with_item("  - id: kr\n    label: Keyring\n    handler: gpg-keyring\n")
                .replacen("schema: 1", "schema: 2", 1),
            false,
        ),
        (
            "handler on unmovable",
            with_item(
                "  - id: kr\n    label: Keyring\n    unmovable: OS-bound\n    handler: gpg-keyring\n",
            )
            .replacen("schema: 1", "schema: 2", 1),
            false,
        ),
    ];
    out.extend(extra.into_iter().map(|(n, y, w)| (format!("v2 {n}"), y, w)));
    out
}
