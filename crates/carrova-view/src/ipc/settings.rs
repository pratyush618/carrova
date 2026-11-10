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
        }
    }
}
