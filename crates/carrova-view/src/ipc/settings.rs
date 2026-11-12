//! User settings the bridge persists.

use carrova_core::{ConflictRule, Retention};
use serde::{Deserialize, Serialize};

/// User settings the bridge persists (front-end owns presentation only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Settings {
    /// Check for recipe updates on launch and daily.
    pub auto_check_recipes: bool,
    /// Check for app updates on launch (never installs without asking).
    pub auto_check_app: bool,
    /// Theme override; `None` follows the system.
    pub theme: Option<Theme>,
    /// Reduced motion override; `None` follows the system.
    pub reduce_motion: Option<bool>,
    /// Suggestions every new move starts from (each move can change them).
    /// Settings saved before these existed read as the defaults.
    #[serde(default)]
    pub move_defaults: MoveDefaults,
    /// Open the Send screen in Manual mode.
    #[serde(default)]
    pub start_in_manual: bool,
    /// The one-time "Send with Carrova" Home card was answered.
    #[serde(default)]
    pub file_manager_asked: bool,
    /// Whether Carrova keeps running in the background.
    #[serde(default)]
    pub background: BackgroundSettings,
    /// What Carrova tells you about, and how loudly.
    #[serde(default)]
    pub alerts: AlertSettings,
    /// Known devices and moves between them.
    #[serde(default)]
    pub nearby: NearbySettings,
}

/// Known-device behaviour. Missing fields read as the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(default)]
pub struct NearbySettings {
    /// Remember devices you move with using a code, so they are known
    /// next time (feeds the engine's remember option).
    pub remember: bool,
    /// Let known devices ask to send you something without a code.
    pub accept_known: bool,
    /// Keep a Wake-on-LAN hint (the device's network address) for known
    /// devices.
    pub wol_hints: bool,
}

impl Default for NearbySettings {
    fn default() -> Self {
        Self {
            remember: true,
            accept_known: true,
            wol_hints: false,
        }
    }
}

/// Background behaviour. Missing fields read as the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(default)]
pub struct BackgroundSettings {
    /// Keep running in the tray when the window closes.
    pub keep_running: bool,
    /// Start in the background when you sign in.
    pub start_at_login: bool,
    /// Release the window's memory while it is hidden.
    pub lean: bool,
}

impl Default for BackgroundSettings {
    fn default() -> Self {
        Self {
            keep_running: true,
            start_at_login: false,
            lean: true,
        }
    }
}

/// Alert preferences. Missing fields read as the defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(default)]
pub struct AlertSettings {
    /// Use system notifications at all.
    pub system: bool,
    /// Wake the screen for a popup.
    pub wake_screen: bool,
    /// Show device names while the screen is locked.
    pub names_on_lock_screen: bool,
    /// Pop-ups are held back until this Unix second; `None` = not paused.
    pub paused_until: Option<u64>,
    /// Someone wants to send you something.
    pub requests: AlertChannel,
    /// A move finished or needs you.
    pub moves: AlertChannel,
    /// A device appeared nearby.
    pub nearby: AlertChannel,
    /// An update is available.
    pub updates: AlertChannel,
}

impl Default for AlertSettings {
    fn default() -> Self {
        Self {
            system: true,
            wake_screen: true,
            names_on_lock_screen: false,
            paused_until: None,
            requests: AlertChannel::PopupAndBell,
            moves: AlertChannel::PopupAndBell,
            nearby: AlertChannel::BellOnly,
            updates: AlertChannel::BellOnly,
        }
    }
}

/// How loudly one kind of alert shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum AlertChannel {
    /// A pop-up and an entry in the bell list.
    PopupAndBell,
    /// Only an entry in the bell list.
    BellOnly,
}

/// The user's suggested defaults for a new move: they pre-fill the send
/// selection and the receiver's review, where they stay editable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct MoveDefaults {
    /// What to do with files that already exist on the receiver
    /// (pre-fills [`carrova_core::PlanOptions::default_rule`]).
    pub conflict_rule: ConflictRule,
    /// How long undo points are kept (pre-fills
    /// [`carrova_core::PlanOptions::retention`]).
    pub retention: Retention,
    /// Tick cache folders when choosing what to send.
    pub include_caches: bool,
}

/// Theme override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Theme {
    /// Light.
    Light,
    /// Dark.
    Dark,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            auto_check_recipes: true,
            auto_check_app: true,
            theme: None,
            reduce_motion: None,
            move_defaults: MoveDefaults::default(),
            start_in_manual: false,
            file_manager_asked: false,
            background: BackgroundSettings::default(),
            alerts: AlertSettings::default(),
            nearby: NearbySettings::default(),
        }
    }
}
