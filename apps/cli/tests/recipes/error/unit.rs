use super::*;

fn not_found() -> BundleError {
    BundleError::Fetch {
        name: "index.json".into(),
        source: FetchError::NotFound,
    }
}

#[test]
fn missing_recipes_point_at_recipes_update() {
    let msg = explain(BundleError::NoRecipes, Action::Load).to_string();
    assert!(msg.contains("carrova recipes update"), "{msg}");
}

#[test]
fn a_bundle_not_published_yet_suggests_retrying_shortly() {
    let msg = retry_later("https://x/").to_string();
    assert!(msg.contains("try again in a minute"), "{msg}");
    assert!(msg.contains("https://x/"), "{msg}");
}

#[test]
fn an_unusable_update_url_is_called_invalid() {
    let err = BundleError::Fetch {
        name: "latest.json".into(),
        source: FetchError::Config("http://x is not an https:// URL".into()),
    };
    let msg = explain(err, Action::Update("http://x")).to_string();
    assert_eq!(msg, "invalid recipe bundle URL");
}

#[test]
fn not_found_while_loading_keeps_its_own_text() {
    let msg = explain(not_found(), Action::Load).to_string();
    assert_eq!(msg, not_found().to_string());
}
