//! The SIMs a dual-SIM phone offers to send a text or place a call from.

use carrova_link::Feature;
use carrova_platform::phone::Sim;
use serde::Serialize;

/// One SIM a picker offers. `Debug` prints no name.
#[derive(Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SimChoice {
    /// What a send or a dial names.
    pub id: i32,
    /// Its slot as the phone numbers it: 1 for the first ("SIM 1").
    pub slot: u16,
    /// The name the phone shows for it (often the carrier's).
    pub name: String,
    /// The phone's default for this use: preselected for a new text or call.
    pub default: bool,
}

private_debug!(SimChoice);

/// What a picker offers for `feature`'s use (texts for
/// [`Feature::Messages`], calls otherwise), by slot: nothing with fewer
/// than two SIMs, so the picker hides and the phone's default is used.
/// Exactly one choice is the default: the phone's, else the first (the
/// phone asks every time).
#[must_use]
pub fn sim_choices(sims: &[Sim], feature: Feature) -> Vec<SimChoice> {
    if sims.len() < 2 {
        return Vec::new();
    }
    let preset = |s: &Sim| {
        if feature == Feature::Messages {
            s.sms_default
        } else {
            s.voice_default
        }
    };
    let mut sorted: Vec<&Sim> = sims.iter().collect();
    sorted.sort_by_key(|s| s.slot);
    let default = sorted
        .iter()
        .find(|s| preset(s))
        .or(sorted.first())
        .map(|s| s.id);
    sorted
        .into_iter()
        .map(|s| SimChoice {
            id: s.id,
            slot: u16::from(s.slot) + 1,
            name: s.name.clone(),
            default: Some(s.id) == default,
        })
        .collect()
}
