//! Presentation models of linked-device features, the pairing invite and
//! the clipboard history. Pure (no I/O, no escaping): front-ends escape
//! every peer-provided string themselves.

use carrova_core::Event;
use carrova_core::ecosystem::{
    AskId, AskKind, FileFailure, LinkedPeer, PairInvite, PeerFound, PeerState,
};
use carrova_crypto::identity::{DeviceId, Fingerprint};
use carrova_link::{ClipItem, Feature, FeatureState, Group, PolicyDoc, allowed};
use carrova_platform::OsKind;
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
/// Holds the pairing code: never logged (its `Debug` redacts it).
#[derive(Clone, PartialEq, Eq, Serialize)]
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

impl std::fmt::Debug for PairInviteView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PairInviteView")
            .field("expires_in_s", &self.expires_in_s)
            .finish_non_exhaustive()
    }
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

/// The device found while pairing, for the user's key check before it is
/// linked. The name is already clean; front-ends still escape it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PairPeerView {
    /// Its name.
    pub name: String,
    /// Its OS family (a phone or a computer).
    pub os: OsKind,
    /// Its fingerprint (`7F3A·19C2`), to compare with the one it shows.
    pub fingerprint: String,
}

/// The key-check view of `peer`.
#[must_use]
pub fn pair_peer_view(peer: &PeerFound) -> PairPeerView {
    PairPeerView {
        name: peer.name().to_owned(),
        os: peer.os(),
        fingerprint: peer.fingerprint().to_string(),
    }
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

/// A known device and its link, as the devices list shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LinkedDeviceView {
    /// Its id, for commands.
    pub id: String,
    /// Its local name.
    pub name: String,
    /// Its OS family.
    pub os: OsKind,
    /// Its fingerprint (`7F3A·19C2`).
    pub fingerprint: String,
    /// Whether the link is up.
    pub state: PeerState,
    /// What each feature does on this link.
    pub features: Vec<FeatureRowView>,
}

impl LinkedDeviceView {
    /// Whether `feature` can be used on this link now (both sides allow
    /// it): what offers an action such as "Ring" or "Send clipboard".
    #[must_use]
    pub fn allows(&self, feature: Feature) -> bool {
        self.features
            .iter()
            .any(|row| row.feature == feature && row.effective != FeatureState::Off)
    }
}

/// `peers` as the devices list shows them, in the given order.
#[must_use]
pub fn linked_device_views(peers: &[LinkedPeer]) -> Vec<LinkedDeviceView> {
    peers
        .iter()
        .map(|p| LinkedDeviceView {
            id: p.id.to_hex(),
            name: p.name.clone(),
            os: p.os,
            fingerprint: Fingerprint::of(&p.id).to_string(),
            state: p.state,
            features: feature_rows(&p.local, p.remote.as_ref()),
        })
        .collect()
}

/// What an ask is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum AskTopic {
    /// A file to save in Downloads.
    File,
    /// A link or file to continue here.
    Handoff,
}

/// A device's offer waiting for the user's answer. Names are already safe;
/// front-ends still escape them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct AskView {
    /// What to answer with.
    pub ask_id: AskId,
    /// The offering device's name.
    pub device_name: String,
    /// What it is about.
    pub kind: AskTopic,
    /// The file's name (a file offer, or a handed-off file).
    pub file_name: Option<String>,
    /// The file's size in bytes (a file offer).
    pub size: Option<u64>,
    /// The link's host (a handed-off link); never the full URL.
    pub url_host: Option<String>,
    /// Seconds left to answer; the caller works it out from its clock.
    pub expires_in_s: u32,
}

/// The view of `kind` offered by `device_name`.
#[must_use]
pub fn ask_view(ask: AskId, device_name: &str, kind: &AskKind, expires_in_s: u32) -> AskView {
    let (kind, file_name, size, url_host) = match kind {
        AskKind::File { name, size } => (AskTopic::File, Some(name.clone()), Some(*size), None),
        AskKind::Handoff { url_host, file } => {
            (AskTopic::Handoff, file.clone(), None, url_host.clone())
        }
    };
    AskView {
        ask_id: ask,
        device_name: device_name.to_owned(),
        kind,
        file_name,
        size,
        url_host,
        expires_in_s,
    }
}

/// Where a file transfer stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum TransferState {
    /// Bytes are moving.
    Active,
    /// The file arrived.
    Done,
    /// The file did not arrive.
    Failed {
        /// Why.
        reason: FileFailure,
        /// It can be sent again (an outgoing file whose device left).
        retry: bool,
    },
}

/// One file moving to or from a linked device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct TransferView {
    /// The transfer's id on its device.
    pub id: u64,
    /// The other device's name.
    pub device_name: String,
    /// The file's name; empty until the engine reports it (on arrival).
    pub file_name: String,
    /// Bytes moved so far.
    pub done: u64,
    /// Bytes in the file.
    pub total: u64,
    /// Where it stands.
    pub state: TransferState,
}

/// The transfers seen so far, folded from the engine's `File*` events.
#[derive(Debug, Default)]
pub struct Transfers(Vec<(DeviceId, TransferView)>);

impl Transfers {
    /// The transfers, in the order they first appeared.
    #[must_use]
    pub fn views(&self) -> Vec<TransferView> {
        self.0.iter().map(|(_, v)| v.clone()).collect()
    }

    /// Fold `event`, which concerns the device called `device_name`;
    /// events that are not about files change nothing.
    pub fn apply(&mut self, event: &Event, device_name: &str) {
        match event {
            Event::FileProgress {
                device,
                id,
                done,
                total,
            } => {
                let view = self.entry(*device, *id, device_name);
                view.done = *done;
                view.total = *total;
            }
            Event::FileSaved { device, id, path } => {
                let view = self.entry(*device, *id, device_name);
                view.done = view.total;
                view.state = TransferState::Done;
                if let Some(name) = path.file_name() {
                    view.file_name = name.to_string_lossy().into_owned();
                }
            }
            Event::FileFailed {
                device,
                id,
                reason,
                retry,
            } => {
                self.entry(*device, *id, device_name).state = TransferState::Failed {
                    reason: *reason,
                    retry: *retry,
                };
            }
            _ => {}
        }
    }

    fn entry(&mut self, device: DeviceId, id: u64, device_name: &str) -> &mut TransferView {
        let at = self
            .0
            .iter()
            .position(|(d, v)| *d == device && v.id == id)
            .unwrap_or_else(|| {
                self.0.push((
                    device,
                    TransferView {
                        id,
                        device_name: device_name.to_owned(),
                        file_name: String::new(),
                        done: 0,
                        total: 0,
                        state: TransferState::Active,
                    },
                ));
                self.0.len() - 1
            });
        &mut self.0[at].1
    }
}
