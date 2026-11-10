//! What captures/applies an item instead of raw file copies: a built-in
//! [`Handler`] or a WASM plugin ([`HandlerRef`]).

use std::fmt;

use serde::de::value::MapAccessDeserializer;
use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::PluginId;

/// Built-in Rust handlers an item may name (closed set; recipes stay data).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Handler {
    /// GnuPG public keys + ownertrust via `gpg --export` / `--import`.
    GpgKeyring,
}

impl Handler {
    /// Every handler, in declaration order.
    pub const ALL: &'static [Self] = &[Self::GpgKeyring];

    /// Stable wire name (same as the YAML spelling), e.g. `gpg-keyring`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::GpgKeyring => "gpg-keyring",
        }
    }

    /// Inverse of [`Handler::as_str`].
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|h| h.as_str() == name)
    }
}

impl fmt::Display for Handler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An item's handler as a recipe names it.
///
/// YAML: a built-in by name (`handler: gpg-keyring`, schema 2+) or a plugin
/// (`handler: { plugin: <id> }`, schema 3+; the version rule is checked by
/// [`crate::validate`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HandlerRef {
    /// A handler compiled into Carrova.
    Builtin(Handler),
    /// A WASM plugin shipped in the signed recipe bundle.
    Plugin(PluginId),
}

impl From<Handler> for HandlerRef {
    fn from(handler: Handler) -> Self {
        Self::Builtin(handler)
    }
}

/// `gpg-keyring` for a built-in, `plugin:<id>` for a plugin.
impl fmt::Display for HandlerRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Builtin(h) => h.fmt(f),
            Self::Plugin(id) => write!(f, "plugin:{id}"),
        }
    }
}

/// Map form of [`HandlerRef::Plugin`]: `{ plugin: <id> }`.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PluginForm<T> {
    plugin: T,
}

impl Serialize for HandlerRef {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Builtin(h) => h.serialize(serializer),
            Self::Plugin(plugin) => PluginForm { plugin }.serialize(serializer),
        }
    }
}

impl<'de> Deserialize<'de> for HandlerRef {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(HandlerRefVisitor)
    }
}

struct HandlerRefVisitor;

impl<'de> Visitor<'de> for HandlerRefVisitor {
    type Value = HandlerRef;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a built-in handler name or `{ plugin: <id> }`")
    }

    fn visit_str<E: de::Error>(self, name: &str) -> Result<HandlerRef, E> {
        Handler::from_name(name)
            .map(HandlerRef::Builtin)
            .ok_or_else(|| {
                let known: Vec<&str> = Handler::ALL.iter().map(|h| h.as_str()).collect();
                E::custom(format_args!(
                    "unknown handler `{name}` (built-in: {}; or `{{ plugin: <id> }}`)",
                    known.join(", ")
                ))
            })
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<HandlerRef, A::Error> {
        let form = PluginForm::<PluginId>::deserialize(MapAccessDeserializer::new(map))?;
        Ok(HandlerRef::Plugin(form.plugin))
    }
}
