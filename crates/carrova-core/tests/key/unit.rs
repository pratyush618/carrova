use carrova_manifest::ItemFlags;

use super::*;

fn manifest_item(recipe: Option<&str>, id: &str) -> ManifestItem {
    ManifestItem {
        recipe_id: recipe.map(str::to_owned),
        item_id: id.into(),
        label: String::new(),
        flags: ItemFlags::default(),
        entries: Vec::new(),
        handler: None,
        artifacts: Vec::new(),
        source_paths: Vec::new(),
    }
}

#[test]
fn keys_keep_their_text_formats() {
    assert_eq!(ItemKey::recipe("git", "config"), "git/config");
    let raw = RootedPath::new(SemanticRoot::UserData, "foo/bar");
    assert_eq!(ItemKey::raw(raw), "raw:user_data/foo/bar");
    assert_eq!(raw.to_string(), "user_data:foo/bar");
    assert_eq!(raw.raw_item_id(), "user_data/foo/bar");
    assert_eq!(
        ItemKey::of(&manifest_item(Some("git"), "config")),
        "git/config"
    );
    assert_eq!(ItemKey::of(&manifest_item(None, "home/.x")), "home/.x");
}

#[test]
fn selection_keys_parse_back() {
    assert_eq!(
        ItemKey::from("git/config").target(),
        Some(ItemTarget::Recipe {
            recipe: "git",
            item: "config"
        })
    );
    assert_eq!(
        ItemKey::from("raw:home/.x/y").target(),
        Some(ItemTarget::Raw(RootedPath::new(SemanticRoot::Home, ".x/y")))
    );
    assert_eq!(ItemKey::from("raw:nowhere/x").target(), None);
    assert_eq!(ItemKey::from("raw:home/").target(), None);
    assert_eq!(ItemKey::from("plain").target(), None);
}

#[test]
fn rooted_paths_round_trip_and_nest() {
    let p = RootedPath::parse("home:.config/git").unwrap();
    assert_eq!(p, RootedPath::new(SemanticRoot::Home, ".config/git"));
    assert_eq!(p.to_string(), "home:.config/git");
    assert_eq!(RootedPath::parse("home:"), None);
    assert_eq!(RootedPath::parse("attic:x"), None);
    assert_eq!(RootedPath::parse("no-colon"), None);
    let base = RootedPath::new(SemanticRoot::Home, ".config");
    assert!(p.is_within(base));
    assert!(!RootedPath::new(SemanticRoot::Home, ".configx").is_within(base));
    assert!(!RootedPath::new(SemanticRoot::UserConfig, ".config/git").is_within(base));
}

#[test]
fn serde_is_the_plain_string() {
    let k = ItemKey::recipe("git", "config");
    assert_eq!(serde_json::to_string(&k).unwrap(), r#""git/config""#);
    let back: ItemKey = serde_json::from_str(r#""raw:home/x""#).unwrap();
    assert_eq!(back, "raw:home/x");
    let artifact = ArtifactId {
        item: k,
        name: "keys".into(),
    };
    assert_eq!(artifact.to_string(), "git/config/keys");
}
