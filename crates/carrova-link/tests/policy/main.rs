//! Public-API tests for the feature policy and two-sided gate.

#[path = "../support/mod.rs"]
mod support;

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
    for f in Feature::PHASE2 {
        let states = match f.group() {
            Some(Group::Quiet) => [On, Off],
            Some(Group::Ask) => [Ask, Off],
            None => unreachable!("phase 2 features have a group"),
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
}

#[test]
fn phase2_features_are_quiet() {
    for f in [Feature::Notifications, Feature::Messages, Feature::Calls] {
        assert_eq!(f.group(), Some(Group::Quiet), "{f:?}");
        assert!(Feature::PHASE2.contains(&f), "{f:?}");
        assert_eq!(allowed(&doc(f, On), &doc(f, On), f), On);
        assert!(matches!(
            doc(f, Ask).validated(),
            Err(LinkError::InvalidState { .. })
        ));
    }
}

#[test]
fn screen_and_input_stay_reserved() {
    let all_on = PolicyDoc {
        version: 1,
        features: [Feature::Screen, Feature::Input]
            .into_iter()
            .map(|f| (f, On))
            .collect(),
    };
    for f in [Feature::Screen, Feature::Input] {
        assert_eq!(f.group(), None);
        assert!(!Feature::PHASE2.contains(&f));
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
            Feature::Ring,
            Feature::Notifications,
            Feature::Messages,
            Feature::Calls
        ]
        .map(|f| phone.get(f)),
        [On, Ask, Ask, On, On, Off, Off]
    );
    let computer = PolicyDoc::computer_defaults();
    assert!(Feature::PHASE2.iter().all(|&f| computer.get(f) == Off));
    assert!(phone.validated().is_ok() && computer.validated().is_ok());
}

#[test]
fn phone_defaults_match_spec() {
    let got = serde_json::to_value(PolicyDoc::phone_defaults()).unwrap();
    let want: serde_json::Value = serde_json::from_str(
        r#"{"version":1,"features":{"clipboard":"on","files":"ask","handoff":"ask","ring":"on","notifications":"on","messages":"off","calls":"off"}}"#,
    )
    .unwrap();
    assert_eq!(got, want);
}

#[test]
fn forward_compatible_remote_doc() {
    let json = r#"{"version":2,"features":{"clipboard":"on","screen":"on","wallet":"on","ring":"prompt","files":"ask"}}"#;
    let doc: PolicyDoc = serde_json::from_str(json).unwrap();
    assert_eq!(doc.get(Feature::Clipboard), On);
    assert_eq!(doc.get(Feature::Files), Ask);
    assert_eq!(doc.get(Feature::Ring), Off, "unknown state reads off");
    assert_eq!(doc.get(Feature::Screen), Off);
    assert!(
        doc.validated().is_ok(),
        "reserved keys never fail validation"
    );
}

#[test]
fn invalid_group_state_reads_off() {
    let files_on = doc(Feature::Files, On);
    assert_eq!(files_on.get(Feature::Files), Off);
    assert_eq!(allowed(&files_on, &files_on, Feature::Files), Off);
    assert_eq!(doc(Feature::Ring, Ask).get(Feature::Ring), Off);
}

#[test]
fn policy_lane_roundtrip_drops_unknown_features() {
    use carrova_link::{LaneKind, accept_lane, open_lane, recv_policy, send_policy};
    use serde::Serialize;

    #[derive(Serialize)]
    struct Newer {
        version: u32,
        features: std::collections::BTreeMap<&'static str, &'static str>,
    }

    let (a, b) = support::linked();
    std::thread::scope(|s| {
        let peer = s.spawn(|| {
            let (kind, mut stream) = accept_lane(&b, &support::Open, support::T).unwrap();
            assert_eq!(kind, LaneKind::Policy);
            let first = recv_policy(&mut stream, support::T).unwrap();
            // A newer peer's document: unknown feature and state are tolerated.
            stream
                .send(&Newer {
                    version: 2,
                    features: [("clipboard", "on"), ("teleport", "ask")].into(),
                })
                .unwrap();
            first
        });
        let mut stream = open_lane(&a, LaneKind::Policy).unwrap();
        send_policy(&mut stream, &PolicyDoc::phone_defaults()).unwrap();
        let theirs = recv_policy(&mut stream, support::T).unwrap();
        assert_eq!(peer.join().unwrap(), PolicyDoc::phone_defaults());
        assert_eq!(theirs.get(Feature::Clipboard), On);
        assert_eq!(theirs.features.len(), 1);
    });
}
