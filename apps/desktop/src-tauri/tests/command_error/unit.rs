#![allow(clippy::unwrap_used)]

use std::io;

use carrova_core::{SessionId, State};

use super::*;

/// Engine errors of several classes (the classification itself is the
/// engine's, tested there).
fn engine_errors() -> Vec<CoreError> {
    vec![
        CoreError::Cancelled,
        CoreError::PeerCancelled,
        CoreError::Selection("app/keys: needs confirmation".into()),
        CoreError::PlanBlocked("app running".into()),
        CoreError::Io(io::Error::from(io::ErrorKind::StorageFull)),
        CoreError::PeerUnreachable { waited_secs: 60 },
        CoreError::NoSuchSession(SessionId("20261108-101500-a1b2c3".into())),
        CoreError::InvalidTransition {
            from: State::Done,
            to: State::Planned,
        },
        CoreError::NotApproved,
    ]
}

#[test]
fn engine_errors_take_the_engine_class_and_hint() {
    for error in engine_errors() {
        let class = error.class();
        let text = error.to_string();
        let mapped = CommandError::from(error);
        assert_eq!(mapped.bridge().code, ErrorCode::from(class.kind), "{text}");
        assert_eq!(
            mapped.bridge().hint.as_deref(),
            class.hint.map(hint_text),
            "{text}"
        );
        assert!(mapped.bridge().message.starts_with(&text), "{text}");
        assert!(mapped.is_engine_failure(), "{text}");
    }
}

#[test]
fn connecting_takes_the_connecting_class() {
    for error in engine_errors() {
        let class = error.class_when_connecting();
        let mapped = CommandError::when_connecting(&error);
        assert_eq!(mapped.bridge().code, ErrorCode::from(class.kind));
    }
}

#[test]
fn bridge_refusals_are_not_engine_failures() {
    assert!(!CommandError::no_session("gone").is_engine_failure());
    assert!(!CommandError::from(BundleError::NoRecipes).is_engine_failure());
    assert!(!CommandError::from(PassphraseRejected::Empty).is_engine_failure());
}

#[test]
fn bundle_errors_are_recipe_or_network_problems() {
    let no_recipes = CommandError::from(BundleError::NoRecipes);
    assert_eq!(no_recipes.bridge().code, ErrorCode::Recipes);
    assert!(no_recipes.bridge().hint.is_some());
    let offline = CommandError::from(BundleError::Fetch {
        name: "index.json".into(),
        source: FetchError::Transport("dns".into()),
    });
    assert_eq!(offline.bridge().code, ErrorCode::Network);
}

#[test]
fn errors_serialise_as_bridge_errors() {
    let json = serde_json::to_value(CommandError::from(CoreError::Cancelled)).unwrap();
    assert_eq!(
        json,
        serde_json::json!({ "code": "cancelled", "message": "cancelled", "hint": null })
    );
}

#[test]
fn the_message_is_the_cause_chain_without_repeats() {
    let error = CommandError::from(CoreError::Io(io::Error::other("disk on fire")));
    assert_eq!(error.bridge().message, "I/O error: disk on fire");
    assert_eq!(error.to_string(), error.bridge().message);
}
