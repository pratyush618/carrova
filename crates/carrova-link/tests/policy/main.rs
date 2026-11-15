//! Public-API tests for the feature policy and two-sided gate.

use FeatureState::{Ask, Off, On};
use carrova_link::{Feature, FeatureState, Group, LinkError, PolicyDoc, allowed};

fn doc(f: Feature, s: FeatureState) -> PolicyDoc {
    PolicyDoc {
        version: 1,
        features: [(f, s)].into(),
    }
}

#[test]
fn gate_requires_both_sides() {
    for f in Feature::PHASE1 {
        let states = match f.group() {
            Group::Quiet => [On, Off],
            Group::Ask => [Ask, Off],
        };
        for local in states {
            for remote in states {
                let want = if local == Off || remote == Off {
                    Off
                } else {
                    local
                };
                assert_eq!(
                    allowed(&doc(f, local), &doc(f, remote), f),
                    want,
                    "{f:?} {local:?} {remote:?}"
                );
            }
        }
    }
    // Mixed On/Ask: the stricter side wins.
    let (a, b) = (doc(Feature::Files, On), doc(Feature::Files, Ask));
    assert_eq!(allowed(&a, &b, Feature::Files), Ask);
    assert_eq!(allowed(&b, &a, Feature::Files), Ask);
}

#[test]
fn reserved_features_read_off() {
    let all_on = PolicyDoc {
        version: 1,
        features: [
            Feature::Notifications,
            Feature::Messages,
            Feature::Calls,
            Feature::Screen,
            Feature::Input,
        ]
        .into_iter()
        .map(|f| (f, On))
        .collect(),
    };
    for f in [
        Feature::Notifications,
        Feature::Messages,
        Feature::Calls,
        Feature::Screen,
        Feature::Input,
    ] {
        assert_eq!(all_on.get(f), Off);
        assert_eq!(allowed(&all_on, &all_on, f), Off);
    }
}

#[test]
fn quiet_feature_rejects_ask() {
    assert!(matches!(
        doc(Feature::Clipboard, Ask).validated(),
        Err(LinkError::InvalidState { .. })
    ));
    assert!(matches!(
        doc(Feature::Files, On).validated(),
        Err(LinkError::InvalidState { .. })
    ));
    assert!(doc(Feature::Clipboard, On).validated().is_ok());
    assert!(doc(Feature::Files, Ask).validated().is_ok());
}

#[test]
fn defaults_match_spec() {
    let phone = PolicyDoc::phone_defaults();
    assert_eq!(
        [
            Feature::Clipboard,
            Feature::Files,
            Feature::Handoff,
            Feature::Ring
        ]
        .map(|f| phone.get(f)),
        [On, Ask, Ask, On]
    );
    let computer = PolicyDoc::computer_defaults();
    assert!(Feature::PHASE1.iter().all(|&f| computer.get(f) == Off));
    assert!(phone.validated().is_ok() && computer.validated().is_ok());
}

#[test]
fn policy_doc_json_shape() {
    let got = serde_json::to_value(PolicyDoc::phone_defaults()).unwrap();
    let want: serde_json::Value = serde_json::from_str(
        r#"{"version":1,"features":{"clipboard":"on","files":"ask","handoff":"ask","ring":"on"}}"#,
    )
    .unwrap();
    assert_eq!(got, want);
}
