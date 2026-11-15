//! Ecosystem view-models (public API).
#![allow(clippy::unwrap_used)]

use std::time::Duration;

use carrova_core::Ticket;
use carrova_core::ecosystem::PairInvite;
use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::PairingCode;
use carrova_link::{ClipItem, Feature, FeatureState, PolicyDoc, allowed};
use carrova_view::ecosystem::{
    ClipKind, FeatureGroup, FeatureId, FeatureStateView, PREVIEW_CHARS, clip_history_view,
    feature_rows, pair_invite_view,
};
use carrova_view::lan::code_view;

fn doc(entries: &[(Feature, FeatureState)]) -> PolicyDoc {
    PolicyDoc {
        version: 1,
        features: entries.iter().copied().collect(),
    }
}

#[test]
fn rows_list_phase_one_features_in_spec_order() {
    let rows = feature_rows(&PolicyDoc::phone_defaults(), None);
    let order: Vec<_> = rows.iter().map(|r| r.feature).collect();
    assert_eq!(
        order,
        [
            FeatureId::Clipboard,
            FeatureId::Ring,
            FeatureId::Files,
            FeatureId::Handoff
        ]
    );
    assert_eq!(rows[0].group, FeatureGroup::Quiet);
    assert_eq!(rows[2].group, FeatureGroup::Ask);
}

#[test]
fn rows_show_remote_off() {
    let local = PolicyDoc::phone_defaults();
    let remote = doc(&[(Feature::Clipboard, FeatureState::On)]);
    let rows = feature_rows(&local, Some(&remote));
    let ring = rows.iter().find(|r| r.feature == FeatureId::Ring).unwrap();
    assert_eq!(ring.local, FeatureStateView::On);
    assert_eq!(ring.remote, Some(FeatureStateView::Off));
    assert_eq!(ring.effective, FeatureStateView::Off);
}

#[test]
fn rows_without_a_remote_document_flow_nothing() {
    for row in feature_rows(&PolicyDoc::phone_defaults(), None) {
        assert_eq!(row.remote, None);
        assert_eq!(row.effective, FeatureStateView::Off, "{:?}", row.feature);
    }
}

#[test]
fn effective_matches_gate() {
    use FeatureState::{Ask, Off, On};
    let sides = [On, Ask, Off];
    for l in sides {
        for r in sides {
            let local = doc(&[(Feature::Clipboard, l), (Feature::Files, l)]);
            let remote = doc(&[(Feature::Clipboard, r), (Feature::Files, r)]);
            for row in feature_rows(&local, Some(&remote)) {
                let f = match row.feature {
                    FeatureId::Clipboard => Feature::Clipboard,
                    FeatureId::Ring => Feature::Ring,
                    FeatureId::Files => Feature::Files,
                    FeatureId::Handoff => Feature::Handoff,
                };
                assert_eq!(
                    row.effective,
                    allowed(&local, &remote, f).into(),
                    "{f:?} {l:?}/{r:?}"
                );
            }
        }
    }
}

#[test]
fn invite_view_has_svg_and_uri() {
    let invite = PairInvite {
        ticket: Ticket {
            code: PairingCode::generate().unwrap(),
            addr: Some("192.168.1.20:4433".parse().unwrap()),
        },
        key: DeviceKey::generate().unwrap().public(),
    };
    let code = code_view(
        &invite.ticket,
        carrova_core::CodeOffer::First,
        Duration::from_secs(90),
    );
    let view = pair_invite_view(&invite, code).unwrap();
    assert_eq!(view.uri, invite.to_uri());
    assert!(view.qr_svg.contains("<svg"), "{}", view.qr_svg);
    assert_eq!(view.expires_in_s, 90);
    assert_eq!(view.code.code, invite.ticket.code.to_string());
}

#[test]
fn history_preview_truncates_120() {
    let long = "a".repeat(500);
    let view = clip_history_view([(&ClipItem::Text(long), 7)]);
    assert_eq!(view.items[0].preview.chars().count(), PREVIEW_CHARS);
    assert_eq!(view.items[0].kind, ClipKind::Text);
    assert_eq!(view.items[0].at, 7);
}

#[test]
fn history_preview_cuts_on_char_boundaries() {
    let multi = "é".repeat(PREVIEW_CHARS + 5);
    let view = clip_history_view([(&ClipItem::Text(multi), 0)]);
    assert_eq!(view.items[0].preview, "é".repeat(PREVIEW_CHARS));
}

#[test]
fn short_text_is_kept_whole_and_images_show_no_content() {
    let png = ClipItem::Png(vec![1, 2, 3]);
    let text = ClipItem::Text("hi".into());
    let view = clip_history_view([(&text, 2), (&png, 1)]);
    assert_eq!(view.items[0].preview, "hi");
    assert_eq!(view.items[1].kind, ClipKind::Image);
    assert!(view.items[1].preview.is_empty());
}
