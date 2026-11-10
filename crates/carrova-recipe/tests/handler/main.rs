//! Handler references: built-in names and validated plugin ids.

#![allow(clippy::unwrap_used)]

use carrova_recipe::{Handler, HandlerRef, PluginId, PluginIdError};

fn parse(yaml: &str) -> Result<HandlerRef, String> {
    serde_yaml_ng::from_str(yaml).map_err(|e| e.to_string())
}

#[test]
fn builtin_name_parses_as_builtin() {
    assert_eq!(
        parse("gpg-keyring"),
        Ok(HandlerRef::Builtin(Handler::GpgKeyring))
    );
}

#[test]
fn plugin_map_parses_as_plugin() {
    assert_eq!(
        parse("{ plugin: vscode-ext }"),
        Ok(HandlerRef::Plugin(PluginId::new("vscode-ext").unwrap()))
    );
}

#[test]
fn unknown_builtin_names_the_known_ones() {
    let err = parse("gpg-export").unwrap_err();
    assert!(err.contains("unknown handler `gpg-export`"), "{err}");
    assert!(err.contains("gpg-keyring"), "{err}");
}

#[test]
fn plugin_map_rejects_bad_ids_and_extra_keys() {
    assert!(parse("{ plugin: Bad }").unwrap_err().contains("a-z"));
    assert!(parse("{ plugin: x, version: 1 }").is_err());
    assert!(parse("{ builtin: gpg-keyring }").is_err());
    assert!(parse("[gpg-keyring]").is_err());
}

#[test]
fn both_forms_roundtrip_through_yaml() {
    for h in [
        HandlerRef::Builtin(Handler::GpgKeyring),
        HandlerRef::Plugin(PluginId::new("a-1").unwrap()),
    ] {
        let text = serde_yaml_ng::to_string(&h).unwrap();
        assert_eq!(parse(&text), Ok(h));
    }
}

#[test]
fn display_distinguishes_plugins_from_builtins() {
    assert_eq!(
        HandlerRef::from(Handler::GpgKeyring).to_string(),
        "gpg-keyring"
    );
    assert_eq!(
        HandlerRef::Plugin(PluginId::new("gpg-keyring").unwrap()).to_string(),
        "plugin:gpg-keyring"
    );
}

#[test]
fn plugin_id_accepts_lowercase_digits_and_dashes_up_to_64() {
    for ok in [
        "a",
        "0",
        "-",
        "vscode-ext-2",
        &"z".repeat(PluginId::MAX_LEN),
    ] {
        assert_eq!(ok.parse::<PluginId>().unwrap().as_str(), ok);
    }
}

#[test]
fn plugin_id_rejects_empty_long_and_foreign_characters() {
    assert_eq!(PluginId::new(""), Err(PluginIdError::Empty));
    assert_eq!(
        PluginId::new(&"z".repeat(65)),
        Err(PluginIdError::TooLong { len: 65 })
    );
    for bad in ["A", "a_b", "a b", "a/b", "é", "a.b"] {
        assert_eq!(
            PluginId::new(bad),
            Err(PluginIdError::InvalidChar { id: bad.into() }),
            "{bad}"
        );
    }
}

#[test]
fn plugin_id_deserialize_validates() {
    assert!(serde_yaml_ng::from_str::<PluginId>("ok-id").is_ok());
    assert!(serde_yaml_ng::from_str::<PluginId>("Not_Ok").is_err());
}
