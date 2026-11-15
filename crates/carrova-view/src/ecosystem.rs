//! Presentation models of linked-device features, the pairing invite and
//! the clipboard history. Pure (no I/O, no escaping): front-ends escape
//! every peer-provided string themselves.

use carrova_core::ecosystem::PairInvite;
use carrova_link::{ClipItem, Feature, FeatureState, Group, PolicyDoc, allowed};
use qrcode::QrCode;
use qrcode::render::svg;
use serde::Serialize;

use crate::lan::CodeView;

/// The order the UI lists the Phase 1 features in.
const ROW_ORDER: [Feature; 4] = [
    Feature::Clipboard,
    Feature::Ring,
    Feature::Files,
    Feature::Handoff,
];

/// Characters of a text clip shown in the history.
pub const PREVIEW_CHARS: usize = 120;

/// One feature on a link: both sides' settings and what actually flows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct FeatureRowView {
    /// The feature.
    pub feature: Feature,
    /// Which states it accepts.
    pub group: Group,
    /// This computer's setting.
    pub local: FeatureState,
    /// The other device's setting; `None` until its policy has arrived.
    pub remote: Option<FeatureState>,
    /// What the two-sided gate allows; `Off` while the remote is unknown.
    pub effective: FeatureState,
}

/// The Phase 1 feature rows of a link, in display order.
#[must_use]
pub fn feature_rows(local: &PolicyDoc, remote: Option<&PolicyDoc>) -> Vec<FeatureRowView> {
    ROW_ORDER
        .into_iter()
        .filter_map(|feature| {
            Some(FeatureRowView {
                feature,
                group: feature.group()?,
                local: local.get(feature),
                remote: remote.map(|r| r.get(feature)),
                effective: remote.map_or(FeatureState::Off, |r| allowed(local, r, feature)),
            })
        })
        .collect()
}

/// A pairing invite as the UI shows it: link text, QR code and fallback code.
///
/// Holds the pairing code: never logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PairInviteView {
    /// The `carrova://pair?...` text the QR code carries.
    pub uri: String,
    /// The QR code as a standalone SVG document.
    pub qr_svg: String,
    /// The code to type when scanning is not possible.
    pub code: CodeView,
    /// Seconds until the invite stops working.
    pub expires_in_s: u32,
}

/// The QR code could not be drawn.
#[derive(Debug, thiserror::Error)]
#[error("invite does not fit a QR code")]
pub struct QrError(#[from] qrcode::types::QrError);

/// The view of `invite`, with `code` as its typed fallback.
pub fn pair_invite_view(invite: &PairInvite, code: CodeView) -> Result<PairInviteView, QrError> {
    let uri = invite.to_uri();
    let qr_svg = QrCode::new(&uri)?
        .render::<svg::Color>()
        .min_dimensions(200, 200)
        .build();
    Ok(PairInviteView {
        uri,
        qr_svg,
        expires_in_s: code.expires_in_secs,
        code,
    })
}

/// What a clipboard item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ClipKind {
    /// UTF-8 text.
    Text,
    /// PNG image.
    Image,
}

/// One received clip in the history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipItemView {
    /// What it is.
    pub kind: ClipKind,
    /// The first [`PREVIEW_CHARS`] characters of a text clip; empty for images.
    pub preview: String,
    /// When it arrived, Unix seconds (the history stores no times: the caller supplies them).
    pub at: u64,
}

/// The received clips, newest first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ClipHistoryView {
    /// Newest first.
    pub items: Vec<ClipItemView>,
}

/// The history view of `items` (newest first), each with its arrival time.
#[must_use]
pub fn clip_history_view<'a>(
    items: impl IntoIterator<Item = (&'a ClipItem, u64)>,
) -> ClipHistoryView {
    let items = items
        .into_iter()
        .map(|(item, at)| match item {
            ClipItem::Text(t) => ClipItemView {
                kind: ClipKind::Text,
                preview: t.chars().take(PREVIEW_CHARS).collect(),
                at,
            },
            ClipItem::Png(_) => ClipItemView {
                kind: ClipKind::Image,
                preview: String::new(),
                at,
            },
        })
        .collect();
    ClipHistoryView { items }
}
