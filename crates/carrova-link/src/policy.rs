//! Feature policy documents and the two-sided gate.

use std::collections::BTreeMap;

use serde::{Deserialize, Deserializer, Serialize};

use crate::LinkError;

/// A feature a link can carry. The last five are reserved and always read `Off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Feature {
    /// Clipboard sync.
    Clipboard,
    /// File offers.
    Files,
    /// URL / file handoff.
    Handoff,
    /// Ring a device.
    Ring,
    /// Reserved.
    Notifications,
    /// Reserved.
    Messages,
    /// Reserved.
    Calls,
    /// Reserved.
    Screen,
    /// Reserved.
    Input,
}

/// Whether a feature runs silently or needs a per-offer approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum Group {
    /// States `on` / `off`.
    Quiet,
    /// States `ask` / `off`.
    Ask,
}

/// One feature's setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum FeatureState {
    /// Runs without asking.
    On,
    /// Runs after a per-offer approval.
    Ask,
    /// Disabled; also what any state this version does not know reads as.
    #[serde(other)]
    Off,
}

impl Feature {
    /// Features active in Phase 1.
    pub const PHASE1: [Self; 4] = [Self::Clipboard, Self::Files, Self::Handoff, Self::Ring];

    /// The group that decides which states this feature accepts; `None` for reserved features.
    #[must_use]
    pub fn group(self) -> Option<Group> {
        match self {
            Self::Clipboard | Self::Ring => Some(Group::Quiet),
            Self::Files | Self::Handoff => Some(Group::Ask),
            Self::Notifications | Self::Messages | Self::Calls | Self::Screen | Self::Input => None,
        }
    }
}

/// A device's per-feature policy as stored and sent on the policy lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDoc {
    /// Schema version (1).
    pub version: u32,
    /// Explicit settings; anything missing reads `Off`. Unknown keys are dropped on decode.
    #[serde(deserialize_with = "known_features")]
    pub features: BTreeMap<Feature, FeatureState>,
}

impl PolicyDoc {
    fn with(features: impl IntoIterator<Item = (Feature, FeatureState)>) -> Self {
        Self {
            version: 1,
            features: features.into_iter().collect(),
        }
    }

    /// Setting for `f`; missing and reserved features read `Off`.
    #[must_use]
    pub fn get(&self, f: Feature) -> FeatureState {
        let state = self.features.get(&f).copied().unwrap_or(FeatureState::Off);
        if state_fits(f.group(), state) {
            state
        } else {
            FeatureState::Off
        }
    }

    /// Strict check for local writes: rejects states outside the feature's group
    /// (quiet: on/off, ask: ask/off). Reserved features are ignored.
    pub fn validated(self) -> Result<Self, LinkError> {
        for (&feature, &state) in &self.features {
            let ok = feature.group().is_none() || state_fits(feature.group(), state);
            if !ok {
                return Err(LinkError::InvalidState { feature, state });
            }
        }
        Ok(self)
    }

    /// Defaults for a newly linked phone.
    #[must_use]
    pub fn phone_defaults() -> Self {
        use FeatureState::{Ask, On};
        Self::with([
            (Feature::Clipboard, On),
            (Feature::Files, Ask),
            (Feature::Handoff, Ask),
            (Feature::Ring, On),
        ])
    }

    /// Defaults for an existing known computer: everything off.
    #[must_use]
    pub fn computer_defaults() -> Self {
        Self::with(Feature::PHASE1.map(|f| (f, FeatureState::Off)))
    }
}

/// Effective state of `f` on a link: `Off` if either side is off, else the
/// stricter side (`Ask` beats `On`).
#[must_use]
pub fn allowed(local: &PolicyDoc, remote: &PolicyDoc, f: Feature) -> FeatureState {
    match (local.get(f), remote.get(f)) {
        (FeatureState::Off, _) | (_, FeatureState::Off) => FeatureState::Off,
        (FeatureState::Ask, _) | (_, FeatureState::Ask) => FeatureState::Ask,
        _ => FeatureState::On,
    }
}

/// Whether `state` is valid for a feature of `group`; reserved features (`None`) never are.
fn state_fits(group: Option<Group>, state: FeatureState) -> bool {
    matches!(
        (group, state),
        (Some(_), FeatureState::Off)
            | (Some(Group::Quiet), FeatureState::On)
            | (Some(Group::Ask), FeatureState::Ask)
    )
}

/// Decodes the feature map, dropping keys this version does not know.
fn known_features<'de, D: Deserializer<'de>>(
    d: D,
) -> Result<BTreeMap<Feature, FeatureState>, D::Error> {
    #[derive(Deserialize, PartialEq, Eq, PartialOrd, Ord)]
    #[serde(untagged)]
    enum Key {
        Known(Feature),
        Unknown(String),
    }
    let raw = BTreeMap::<Key, FeatureState>::deserialize(d)?;
    Ok(raw
        .into_iter()
        .filter_map(|(k, v)| match k {
            Key::Known(f) => Some((f, v)),
            Key::Unknown(_) => None,
        })
        .collect())
}
