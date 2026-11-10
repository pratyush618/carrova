//! Plan blockers name running apps by their display name, travel as
//! tagged objects, and plans written before names were recorded still
//! read.
#![allow(clippy::unwrap_used)]

use carrova_core::plan::Blocker;

#[test]
fn a_running_app_blocker_shows_the_display_name_and_keeps_the_id() {
    let blocker = Blocker::RunningApp {
        app: "code".into(),
        name: "Visual Studio Code".into(),
    };
    let text = blocker.to_string();
    assert!(
        text.starts_with("Visual Studio Code (`code`) is running"),
        "{text}"
    );
    let json = serde_json::to_string(&blocker).unwrap();
    assert_eq!(
        json,
        r#"{"kind":"running_app","app":"code","name":"Visual Studio Code"}"#
    );
    assert_eq!(serde_json::from_str::<Blocker>(&json).unwrap(), blocker);
}
