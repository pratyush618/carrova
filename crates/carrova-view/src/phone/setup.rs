//! Where the call-audio setup and the phone's permission prompts stand.

use carrova_platform::phone::{CallAudioStatus, PermState, PhonePerm};
use serde::Serialize;

/// A screen of the call-audio setup ("Calls setup"). `Intro`, `Phone`
/// and `Test` are steps the user walks through; the others follow from
/// [`call_audio_view`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum CallSetupStep {
    /// What setting up does.
    Intro,
    /// Talk on the phone: no setup needed.
    Phone,
    /// Install the one package hands-free audio needs.
    Install,
    /// Pair the phone over Bluetooth.
    Pair,
    /// Check the sound.
    Test,
    /// Set up: calls can use this computer's mic and speakers.
    Done,
    /// Bluetooth is switched off.
    Off,
    /// This computer can't carry call audio.
    NoBt,
}

/// The setup screen this computer's call-audio `status` calls for.
#[must_use]
pub fn call_audio_view(status: CallAudioStatus) -> CallSetupStep {
    match status {
        CallAudioStatus::NoAdapter
        | CallAudioStatus::NoHandsFree
        | CallAudioStatus::Unsupported => CallSetupStep::NoBt,
        CallAudioStatus::BluetoothOff => CallSetupStep::Off,
        CallAudioStatus::NeedsPackage => CallSetupStep::Install,
        CallAudioStatus::NeedsPairing => CallSetupStep::Pair,
        CallAudioStatus::Ready => CallSetupStep::Done,
    }
}

/// What the phone shows for a permission a feature needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PermStep {
    /// Ask for it (Android's prompt).
    Ask,
    /// "Allow restricted settings" first (App info › ⋮), then ask.
    Restricted,
    /// Only a settings page grants it: open that page.
    OpenSettings,
    /// Granted.
    Ok,
}

/// The step for `perm` in `state`.
#[must_use]
pub fn perm_view(perm: PhonePerm, state: PermState) -> PermStep {
    match (state, perm) {
        (PermState::Granted, _) => PermStep::Ok,
        (PermState::Restricted, _) => PermStep::Restricted,
        // Notification access has no prompt: Android grants it on a settings page.
        (PermState::Denied, PhonePerm::NotificationAccess) => PermStep::OpenSettings,
        (
            PermState::Denied,
            PhonePerm::Sms | PhonePerm::Calls | PhonePerm::Contacts | PhonePerm::BluetoothConnect,
        ) => PermStep::Ask,
    }
}
