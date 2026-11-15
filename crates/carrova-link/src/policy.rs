//! Feature policy documents and the two-sided gate.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::LinkError;

/// A feature a link can carry. The last five are reserved and always read `Off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    /// States `on` / `off`.
    Quiet,
    /// States `ask` / `off`.
    Ask,
}

/// One feature's setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeatureState {
    /// Runs without asking.
    On,
    /// Disabled.
    Off,
    /// Runs after a per-offer approval.
    Ask,
}

impl Feature {
    /// Features active in Phase 1.
    pub const PHASE1: [Self; 4] = [Self::Clipboard, Self::Files, Self::Handoff, Self::Ring];

    /// The group that decides which states this feature accepts.
    #[must_use]
    pub fn group(self) -> Group {
        match self {
            Self::Clipboard | Self::Ring => Group::Quiet,
            _ => Group::Ask,
        }
    }
}

/// A device's per-feature policy as stored and sent on the policy lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyDoc {
    /// Schema version (1).
    pub version: u32,
    /// Explicit settings; anything missing reads `Off`.
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
        if Feature::PHASE1.contains(&f) {
            self.features.get(&f).copied().unwrap_or(FeatureState::Off)
        } else {
            FeatureState::Off
        }
    }

    /// Rejects states outside the feature's group (quiet: on/off, ask: ask/off).
    pub fn validated(self) -> Result<Self, LinkError> {
        for (&feature, &state) in &self.features {
            let ok = matches!(
                (feature.group(), state),
                (_, FeatureState::Off)
                    | (Group::Quiet, FeatureState::On)
                    | (Group::Ask, FeatureState::Ask)
            );
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
