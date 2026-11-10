//! Schema v3 cases: every v2 case re-run at `schema: 3` (where v3 changes
//! the verdict, the override says so), the v3-only fields, and v3 fields
//! refused by older schemas.

use super::cases::{BASE, cases_v2, with_item};

/// Base recipe at `schema: 3`.
fn v3() -> String {
    BASE.replacen("schema: 1", "schema: 3", 1)
}

/// `v3()` with the first item's `exclude` line followed by `extra`.
fn with_config(extra: &str) -> String {
    v3().replacen(
        "exclude: [\"**/credentials\"]\n",
        &format!("exclude: [\"**/credentials\"]\n{extra}"),
        1,
    )
}

/// A slot-aligned macOS override of the base item's two paths.
const MACOS_OVERRIDE: &str = "    os:
      macos:
        paths:
          - { root: home, rel: .gitconfig }
          - { root: home, rel: .config/git }
";

/// v2 verdicts that schema 3 changes.
const V3_VERDICTS: [(&str, bool); 1] = [("v2 no linux detect", true)];

/// (name, yaml, expected valid).
pub fn cases_v3() -> Vec<(String, String, bool)> {
    let mut out: Vec<(String, String, bool)> = cases_v2()
        .into_iter()
        .filter(|(name, _, _)| name != "v2 schema 2")
        .map(|(name, yaml, want)| {
            let want = V3_VERDICTS
                .iter()
                .find(|(n, _)| *n == name)
                .map_or(want, |(_, w)| *w);
            let name = name.replacen("v2 ", "v3 ", 1);
            (name, yaml.replacen("schema: 2", "schema: 3", 1), want)
        })
        .collect();
    out.extend(
        valid()
            .into_iter()
            .map(|(n, y)| (format!("v3 {n}"), y, true))
            .chain(
                invalid()
                    .into_iter()
                    .map(|(n, y)| (format!("v3 {n}"), y, false)),
            )
            .chain(
                older_schema_rejects()
                    .into_iter()
                    .map(|(n, y)| (format!("v2 rejects {n}"), y, false)),
            ),
    );
    out
}

fn valid() -> Vec<(&'static str, String)> {
    let r = |from: &str, to: &str| v3().replacen(from, to, 1);
    vec![
        ("schema 3", v3()),
        ("os override", with_config(MACOS_OVERRIDE)),
        (
            "os override with own excludes",
            with_config(&format!(
                "{MACOS_OVERRIDE}        exclude: [\"**/Cache\"]\n"
            )),
        ),
        (
            "os override with no excludes",
            with_config(&format!("{MACOS_OVERRIDE}        exclude: []\n")),
        ),
        (
            "os override for every os",
            with_config(&format!(
                "{MACOS_OVERRIDE}      windows:\n        paths: [{{ root: home, rel: .gitconfig }}, {{ root: user_config, rel: Git }}]\n      linux:\n        paths: [{{ root: home, rel: .gitconfig }}, {{ root: user_config, rel: git }}]\n"
            )),
        ),
        (
            "plugin handler",
            r(
                "label: Global git config",
                "label: Global git config\n    handler: { plugin: git-credential-store }",
            ),
        ),
        (
            "windows-only detect",
            r(
                "  linux:\n    - binary: git\n    - path: { root: home, rel: .gitconfig }\n",
                "  windows: [{ binary: git }]\n",
            ),
        ),
    ]
}

fn invalid() -> Vec<(&'static str, String)> {
    let r = |from: &str, to: &str| v3().replacen(from, to, 1);
    let plugin = |id: &str| {
        r(
            "label: Global git config",
            &format!("label: G\n    handler: {{ plugin: '{id}' }}"),
        )
    };
    vec![
        (
            "no detect for any os",
            r(
                "  linux:\n    - binary: git\n    - path: { root: home, rel: .gitconfig }\n",
                "  linux: []\n  macos: []\n",
            ),
        ),
        ("os unknown os", with_config("    os:\n      bsd: { paths: [] }\n")),
        (
            "os unknown field",
            with_config(&format!("{MACOS_OVERRIDE}        mode: 1\n")),
        ),
        (
            "os paths missing",
            with_config("    os:\n      macos: { exclude: [x] }\n"),
        ),
        (
            "os path parent",
            with_config(&MACOS_OVERRIDE.replacen("rel: .config/git", "rel: ../git", 1)),
        ),
        (
            "os path absolute",
            with_config(&MACOS_OVERRIDE.replacen("rel: .config/git", "rel: /etc/git", 1)),
        ),
        (
            "os path colon",
            with_config(&MACOS_OVERRIDE.replacen("rel: .config/git", "rel: a/C:/x", 1)),
        ),
        (
            "windows path drive after first component",
            with_config(
                "    os:\n      windows:\n        paths: [{ root: home, rel: .gitconfig }, { root: user_config, rel: 'AppData\\C:\\x' }]\n",
            ),
        ),
        (
            "os empty exclude glob",
            with_config(&format!("{MACOS_OVERRIDE}        exclude: ['']\n")),
        ),
        ("plugin id uppercase", plugin("Git")),
        ("plugin id underscore", plugin("git_store")),
        ("plugin id empty", plugin("")),
        ("plugin id too long", plugin(&"a".repeat(65))),
        (
            "plugin extra key",
            r(
                "label: Global git config",
                "label: G\n    handler: { plugin: x, version: 1 }",
            ),
        ),
        (
            "plugin handler on unmovable",
            with_item(
                "  - id: kr\n    label: Keyring\n    unmovable: OS-bound\n    handler: { plugin: kr }\n",
            )
            .replacen("schema: 1", "schema: 3", 1),
        ),
    ]
}

/// v3-only fields in a schema 2 recipe.
fn older_schema_rejects() -> Vec<(&'static str, String)> {
    let v2 = |yaml: String| yaml.replacen("schema: 3", "schema: 2", 1);
    vec![
        ("os override", v2(with_config(MACOS_OVERRIDE))),
        (
            "plugin handler",
            v2(v3().replacen(
                "label: Global git config",
                "label: G\n    handler: { plugin: x }",
                1,
            )),
        ),
    ]
}

/// v3 documents the JSON Schema accepts but the Rust validator rejects
/// with exactly one issue.
pub fn validator_only_v3() -> Vec<String> {
    vec![
        // override with one path for a two-path item (slot misalignment)
        with_config("    os:\n      windows:\n        paths: [{ root: home, rel: .gitconfig }]\n"),
        // sensitive path captured on macOS only (override drops the exclude)
        with_config(&format!("{MACOS_OVERRIDE}        exclude: []\n")).replacen(
            "items:\n",
            "items:\n  - id: creds\n    label: Creds\n    paths: [{ root: home, rel: .config/git/credentials }]\n    flags: { sensitive: true }\n",
            1,
        ),
    ]
}
