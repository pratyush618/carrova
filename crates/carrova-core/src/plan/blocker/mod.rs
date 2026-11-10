//! [`Blocker`]: why apply must not run yet.
//!
//! Serialised as a tagged object (`{"kind": "running_app", …}`), so
//! front-ends read its fields instead of parsing text. Plans written
//! before that stored each blocker as its display text; [`read_blockers`]
//! still reads those (`legacy`).

use std::fmt;

use carrova_platform::OsKind;
use carrova_recipe::PluginId;
use serde::{Deserialize, Deserializer, Serialize};

use crate::key::ItemKey;
use crate::space::SpaceShortfall;

mod legacy;

/// Something the user has to resolve (or explicitly override) before the
/// plan may be applied.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Blocker {
    /// A selected item the receiver treats as sensitive was not confirmed.
    UnconfirmedSensitive {
        /// Item key.
        item: ItemKey,
        /// Why it is sensitive ([`super::Plan::sensitive`]).
        reason: String,
    },
    /// An app that must be closed while its data changes is running.
    RunningApp {
        /// Recipe id.
        app: String,
        /// The app's display name (the recipe's `name`; the id when the
        /// receiver has no such recipe, and in plans written before names
        /// were recorded).
        name: String,
    },
    /// A filesystem the plan writes to is short of free space.
    FreeSpace(SpaceShortfall),
    /// A selected item whose recipe marks it same-OS was captured on
    /// another OS family; the user may override it per item
    /// ([`super::PlanOptions::allow_cross_os`]).
    CrossOs {
        /// Item key.
        item: ItemKey,
        /// OS family the archive was captured on.
        from: OsKind,
        /// OS family of this computer.
        to: OsKind,
    },
    /// A plugin would change the files of selected items and the user has
    /// not allowed it ([`super::PlanOptions::confirmed_plugins`]).
    UnconfirmedPlugin {
        /// Plugin id.
        #[cfg_attr(feature = "ts", ts(as = "String"))]
        plugin: PluginId,
        /// Selected items whose files it would change.
        items: Vec<ItemKey>,
    },
}

impl fmt::Display for Blocker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnconfirmedSensitive { item, reason } => write!(
                f,
                "`{item}` is sensitive ({reason}): confirm it explicitly to import it, or \
                 deselect it"
            ),
            Self::RunningApp { app, name } => write!(
                f,
                "{name} (`{app}`) is running: close it before applying, or override the \
                 requires-closed check"
            ),
            Self::FreeSpace(short) => write!(f, "not enough free space: {short}"),
            Self::CrossOs { item, from, to } => write!(
                f,
                "`{item}` comes from {from} and only works on the same OS ({to} here): \
                 deselect it, or override the same-OS check"
            ),
            Self::UnconfirmedPlugin { plugin, items } => {
                let items: Vec<String> = items.iter().map(|i| format!("`{i}`")).collect();
                write!(
                    f,
                    "plugin `{plugin}` would change the files of {}: review what it may do and \
                     allow it, or deselect those items",
                    items.join(", ")
                )
            }
        }
    }
}

/// One stored blocker: the tagged form, or the display text of plans
/// written before it.
#[derive(Deserialize)]
#[serde(untagged)]
enum Stored {
    Tagged(Blocker),
    Text(String),
}

/// Deserialises [`super::Plan::blockers`] in either stored form (tagged,
/// or the display text older plans hold).
pub(crate) fn read_blockers<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<Blocker>, D::Error> {
    Vec::<Stored>::deserialize(d)?
        .into_iter()
        .map(|stored| match stored {
            Stored::Tagged(blocker) => Ok(blocker),
            Stored::Text(text) => legacy::parse(&text).ok_or_else(|| {
                serde::de::Error::custom(format_args!("not a plan blocker: {text:?}"))
            }),
        })
        .collect()
}

#[cfg(test)]
#[path = "../../../tests/plan/blocker/unit.rs"]
mod tests;
