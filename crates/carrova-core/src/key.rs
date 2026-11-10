//! Typed names for what a session moves: [`ItemKey`] (one item),
//! [`RootedPath`] (one path under a semantic root) and [`ArtifactId`] (one
//! handler artifact of an item). Each owns its text format, so no other
//! module builds or splits these strings.
//!
//! | name           | format                                          |
//! |----------------|-------------------------------------------------|
//! | recipe item    | `<recipe>/<item>`                               |
//! | raw dir (sender selection) | `raw:<root>/<rel>`                  |
//! | raw dir (manifest item id, receiver key) | `<root>/<rel>`        |
//! | path           | `<root>:<rel>`                                  |

use std::borrow::Borrow;
use std::convert::Infallible;
use std::fmt;
use std::str::FromStr;

use carrova_manifest::ManifestItem;
use carrova_platform::SemanticRoot;
use serde::{Deserialize, Serialize};

/// Stable key of one item.
///
/// - A recipe item: `<recipe>/<item>` (both sides).
/// - On the sender, an unrecognised directory selected as a whole:
///   `raw:<root>/<rel>` ([`ItemKey::raw`]).
/// - On the receiver, a manifest item without a recipe: its bare item id
///   (for a raw directory `<root>/<rel>`, the selection key without the
///   prefix; [`ItemKey::of`]).
///
/// Keys from the user or a peer are taken verbatim ([`FromStr`] never
/// fails): whether one names an item is decided by looking it up.
/// Serialised as the plain string; ordered like it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(transparent)]
pub struct ItemKey(String);

/// What a sender's selection key names ([`ItemKey::target`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemTarget<'a> {
    /// Item `item` of recipe `recipe`.
    Recipe {
        /// Recipe id.
        recipe: &'a str,
        /// Item id within the recipe.
        item: &'a str,
    },
    /// An unrecognised directory.
    Raw(RootedPath<'a>),
}

impl ItemKey {
    /// Prefix of a sender's raw-directory selection keys.
    pub const RAW_PREFIX: &'static str = "raw:";

    /// Key of item `item` of recipe `recipe`: `<recipe>/<item>`.
    #[must_use]
    pub fn recipe(recipe: &str, item: &str) -> Self {
        Self(format!("{recipe}/{item}"))
    }

    /// Sender selection key of the unrecognised directory `path`:
    /// `raw:<root>/<rel>`.
    #[must_use]
    pub fn raw(path: RootedPath<'_>) -> Self {
        Self(format!("{}{}", Self::RAW_PREFIX, path.raw_item_id()))
    }

    /// Receiver key of a manifest item: [`Self::recipe`] for recipe items,
    /// else the bare item id.
    #[must_use]
    pub fn of(item: &ManifestItem) -> Self {
        match &item.recipe_id {
            Some(recipe) => Self::recipe(recipe, &item.item_id),
            None => Self(item.item_id.clone()),
        }
    }

    /// The key text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether this is a sender's raw-directory selection key.
    #[must_use]
    pub fn is_raw(&self) -> bool {
        self.0.starts_with(Self::RAW_PREFIX)
    }

    /// What this sender selection key names; `None` if it is malformed
    /// (no `/`, or a raw key with an unknown root or empty path).
    #[must_use]
    pub fn target(&self) -> Option<ItemTarget<'_>> {
        match self.0.strip_prefix(Self::RAW_PREFIX) {
            Some(raw) => RootedPath::parse_raw_item_id(raw).map(ItemTarget::Raw),
            None => {
                let (recipe, item) = self.0.split_once('/')?;
                Some(ItemTarget::Recipe { recipe, item })
            }
        }
    }
}

impl fmt::Display for ItemKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.pad(&self.0)
    }
}

impl FromStr for ItemKey {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_owned()))
    }
}

impl From<&str> for ItemKey {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl From<String> for ItemKey {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<ItemKey> for String {
    fn from(k: ItemKey) -> Self {
        k.0
    }
}

impl Borrow<str> for ItemKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for ItemKey {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<str> for ItemKey {
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

impl PartialEq<&str> for ItemKey {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

/// One handler artifact: artifact `name` of item `item`. Shown as
/// `<item>/<name>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ArtifactId {
    /// Item the artifact belongs to.
    pub item: ItemKey,
    /// Artifact name (from the manifest).
    pub name: String,
}

impl fmt::Display for ArtifactId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.item, self.name)
    }
}

/// A path under a semantic root, borrowed: `<root>:<rel>` (path keys,
/// per-file rules, exclusions, messages). Owned forms stay plain strings
/// (`to_string()`), parsed back with [`RootedPath::parse`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootedPath<'a> {
    /// The semantic root.
    pub root: SemanticRoot,
    /// `/`-separated path relative to the root.
    pub rel: &'a str,
}

impl<'a> RootedPath<'a> {
    /// `rel` under `root`.
    #[must_use]
    pub const fn new(root: SemanticRoot, rel: &'a str) -> Self {
        Self { root, rel }
    }

    /// Parse `<root>:<rel>`; `None` for an unknown root or an empty `rel`.
    #[must_use]
    pub fn parse(key: &'a str) -> Option<Self> {
        let (root, rel) = key.split_once(':')?;
        Self::with_root_name(root, rel)
    }

    /// Item id of a raw directory: `<root>/<rel>` (also how raw
    /// directories are listed).
    #[must_use]
    pub fn raw_item_id(self) -> String {
        format!("{}/{}", self.root, self.rel)
    }

    /// Parse a raw item id `<root>/<rel>` ([`Self::raw_item_id`]).
    #[must_use]
    pub fn parse_raw_item_id(id: &'a str) -> Option<Self> {
        let (root, rel) = id.split_once('/')?;
        Self::with_root_name(root, rel)
    }

    /// Whether this path equals `base` or lies below it (same root,
    /// `/`-separated, lexical).
    #[must_use]
    pub fn is_within(self, base: RootedPath<'_>) -> bool {
        self.root == base.root && rel_within(self.rel, base.rel)
    }

    fn with_root_name(root: &str, rel: &'a str) -> Option<Self> {
        let root = SemanticRoot::ALL.into_iter().find(|r| r.as_str() == root)?;
        (!rel.is_empty()).then_some(Self { root, rel })
    }
}

impl fmt::Display for RootedPath<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.root, self.rel)
    }
}

/// Whether `rel` equals `base` or lies below it (`/`-separated, lexical).
#[must_use]
pub(crate) fn rel_within(rel: &str, base: &str) -> bool {
    rel == base
        || rel
            .strip_prefix(base)
            .is_some_and(|rest| rest.starts_with('/'))
}

#[cfg(test)]
#[path = "../tests/key/unit.rs"]
mod tests;
