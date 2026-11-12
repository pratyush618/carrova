//! User settings the bridge persists, their read-only load for every
//! front-end (the CLI follows the app's Nearby settings), and the save
//! policy that keeps saved wake hints in step with them.

use carrova_core::devices::KnownDevices;
use carrova_core::{ConflictRule, CoreError, Retention};
use carrova_platform::{Platform, PlatformError};
use serde::{Deserialize, Serialize};

/// Settings file name in Carrova's state directory.
pub const SETTINGS_FILE: &str = "desktop-settings.json";

/// The saved settings cannot be read.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// The state directory is unknown.
    #[error(transparent)]
    Platform(#[from] PlatformError),
    /// The file exists but cannot be read.
    #[error("cannot read the settings")]
    Io(#[from] std::io::Error),
    /// The file is not valid settings.
    #[error("the saved settings are not valid")]
    Parse(#[from] serde_json::Error),
}

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

impl NearbySettings {
    /// The saved nearby settings, for a new engine; the defaults if none
    /// were saved yet. Unreadable settings never block a move, but may
    /// hide an opt-out: they read as the defaults with remembering and
    /// wake hints off.
    pub fn saved(platform: &dyn Platform) -> Self {
        Settings::load(platform).map_or_else(
            |error| {
                tracing::warn!(%error, "settings unreadable; not remembering devices");
                Self {
                    remember: false,
                    wol_hints: false,
                    ..Self::default()
                }
            },
            |settings| settings.nearby,
        )
    }
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

impl Settings {
    /// The settings saved in `platform`'s state directory; the defaults if
    /// none were saved yet.
    ///
    /// # Errors
    ///
    /// The file exists but cannot be read or parsed.
    pub fn load(platform: &dyn Platform) -> Result<Self, SettingsError> {
        let path = platform.state_dir()?.join(SETTINGS_FILE);
        match std::fs::read(&path) {
            Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Replace the saved `old` settings with `self` through `write`, keeping
    /// the saved wake hints in step. Run it under the writer's settings
    /// lock. Every save with wake hints off saves the setting, then drops
    /// every saved hint: a move remembered meanwhile asks the saved setting
    /// under the devices lock, so none comes back. Hints are dropped even
    /// if the save fails, unless the saved file still has them on, and one an
    /// earlier save left behind goes with the next. No device store yet
    /// means nothing to drop, and none is made. If the hints cannot be
    /// dropped while turning them off, `old` is written back; while
    /// already off, that is only logged.
    ///
    /// # Errors
    ///
    /// `write` fails (that error wins), or the hints cannot be dropped
    /// while turning them off.
    pub fn save_over<E: From<CoreError>>(
        &self,
        old: &Self,
        platform: &dyn Platform,
        mut write: impl FnMut(&Self) -> Result<(), E>,
    ) -> Result<(), E> {
        let saved = write(self);
        // A failed save may or may not have replaced the file: ask it.
        let left_on =
            saved.is_err() && Settings::load(platform).is_ok_and(|saved| saved.nearby.wol_hints);
        if self.nearby.wol_hints || left_on {
            return saved;
        }
        let cleared = KnownDevices::open_existing(platform)
            .and_then(|k| k.map_or(Ok(()), |k| k.clear_wake_hints(platform)));
        // Logged first: a failed save or rollback below replaces it.
        if let Err(e) = &cleared {
            tracing::warn!(error = %e, "could not drop the wake hints");
        }
        saved?;
        if let Err(e) = cleared
            && old.nearby.wol_hints
        {
            write(old)?;
            return Err(e.into());
        }
        Ok(())
    }
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
