//! `plugin.yaml`: parsing, validation and the consent text.

#![allow(clippy::unwrap_used, clippy::panic)]

use carrova_plugin::{ContextField, MAX_FUEL, MAX_MEMORY_MIB, PluginError, PluginManifest};

const GOOD: &str = "\
schema: 1
id: path-rewrite
version: 1.0.0
description: Rewrites home folder paths inside config files.
context: [source_home, target_home]
limits: { fuel: 1000000, memory_mib: 4 }
";

fn parse(text: &str) -> Result<PluginManifest, PluginError> {
    PluginManifest::parse(text)
}

fn invalid(text: &str) -> String {
    match parse(text) {
        Err(PluginError::Manifest(why)) => why,
        other => panic!("expected an invalid manifest, got {other:?}"),
    }
}

#[test]
fn a_valid_manifest_parses() {
    let m = parse(GOOD).unwrap();
    assert_eq!(m.id.as_str(), "path-rewrite");
    assert_eq!(m.version, "1.0.0");
    assert_eq!(
        m.context,
        vec![ContextField::SourceHome, ContextField::TargetHome]
    );
    assert_eq!(m.limits.fuel, 1_000_000);
    assert_eq!(m.limits.memory_mib, 4);
}

#[test]
fn unknown_fields_are_refused() {
    let text = format!("{GOOD}network: true\n");
    assert!(invalid(&text).contains("network"));
}

#[test]
fn an_unsupported_schema_is_refused() {
    let text = GOOD.replace("schema: 1", "schema: 2");
    assert!(invalid(&text).contains("schema 2"));
}

#[test]
fn an_unknown_context_field_is_refused() {
    let text = GOOD.replace("target_home]", "ssh_keys]");
    assert!(invalid(&text).contains("ssh_keys"));
}

#[test]
fn a_bad_id_is_refused() {
    assert!(invalid(&GOOD.replace("path-rewrite", "Path_Rewrite")).contains("Path_Rewrite"));
}

#[test]
fn limits_above_the_hard_caps_are_refused() {
    let fuel = GOOD.replace("fuel: 1000000", &format!("fuel: {}", MAX_FUEL + 1));
    assert!(invalid(&fuel).contains("fuel"));
    let memory = GOOD.replace(
        "memory_mib: 4",
        &format!("memory_mib: {}", MAX_MEMORY_MIB + 1),
    );
    assert!(invalid(&memory).contains("memory"));
    assert!(invalid(&GOOD.replace("memory_mib: 4", "memory_mib: 0")).contains("memory"));
    assert!(invalid(&GOOD.replace("fuel: 1000000", "fuel: 0")).contains("fuel"));
}

#[test]
fn blank_or_multiline_text_is_refused() {
    assert!(invalid(&GOOD.replace("version: 1.0.0", "version: ''")).contains("version"));
    let multiline = GOOD.replace(
        "description: Rewrites home folder paths inside config files.",
        "description: \"line one\\nline two\"",
    );
    assert!(invalid(&multiline).contains("description"));
}

#[test]
fn a_context_field_listed_twice_is_refused() {
    let text = GOOD.replace("target_home]", "source_home]");
    assert!(invalid(&text).contains("source_home"));
}

#[test]
fn permissions_name_every_declared_context_field_and_the_sandbox() {
    let lines = parse(GOOD).unwrap().permissions();
    let text = lines.join("\n");
    assert!(text.contains("change the content"), "{text}");
    assert!(text.contains("home folder on the old computer"), "{text}");
    assert!(text.contains("home folder on this computer"), "{text}");
    assert!(!text.contains("operating system"), "{text}");
    assert!(text.contains("cannot read or write other files"), "{text}");
}

#[test]
fn run_tools_are_bare_unique_program_names() {
    let m = parse(&format!(
        "{GOOD}run_tools: [sqlite3, jq, python3.12, g++]\n"
    ))
    .unwrap();
    assert_eq!(m.run_tools, ["sqlite3", "jq", "python3.12", "g++"]);
    for bad in [
        "/usr/bin/sqlite3",
        "../sqlite3",
        "bin\\\\tool",
        "'sqlite3 -cmd x'",
        "-rf",
        ".hidden",
        "''",
        "'tool;rm'",
        "'t\\u00e9'",
    ] {
        let why = invalid(&format!("{GOOD}run_tools: [{bad}]\n"));
        assert!(why.contains("run_tools"), "{bad}: {why}");
    }
    let long = "a".repeat(65);
    assert!(invalid(&format!("{GOOD}run_tools: [{long}]\n")).contains("run_tools"));
    assert!(invalid(&format!("{GOOD}run_tools: [jq, jq]\n")).contains("jq"));
}

#[test]
fn permissions_name_each_tool_the_log_and_the_export() {
    let plain = parse(GOOD).unwrap().permissions().join("\n");
    assert!(plain.contains("or run programs"), "{plain}");
    assert!(!plain.contains("log"), "{plain}");

    let text = format!("{GOOD}run_tools: [sqlite3, jq]\nlog: true\nexport: true\n");
    let lines = parse(&text).unwrap().permissions();
    let all = lines.join("\n");
    for tool in ["`sqlite3`", "`jq`"] {
        let line = lines.iter().find(|l| l.contains(tool)).unwrap();
        assert!(line.contains("not sandboxed"), "{line}");
    }
    assert!(
        all.contains("may write diagnostic notes to Carrova's log"),
        "{all}"
    );
    assert!(all.contains("on the old computer"), "{all}");
    assert!(all.contains("before they are sent"), "{all}");
    assert!(all.contains("run any other program"), "{all}");
    assert!(!all.contains("or run programs"), "{all}");
}
