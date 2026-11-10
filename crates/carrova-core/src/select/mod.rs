//! Selection model (sender side). **The user decides everything**: the
//! engine only *suggests*. Every app, item, path and single file can be
//! included or excluded; presets merely pre-fill choices.
//!
//! Items are named by [`ItemKey`]s (recipe items and raw unrecognised
//! dirs); paths inside an item by [`RootedPath`] text (`"<root>:<rel>"`).
//!
//! Front-ends change a selection only through [`SelectionEdit`]s
//! ([`Selection::apply`]): the one set of selection rules every front-end
//! shares.
//!
//! Non-overridable rails: sensitive items start unticked and need an explicit
//! confirmation; unmovable items need `force_unmovable` (shown with reason).

use std::collections::BTreeMap;

use carrova_platform::SemanticRoot;
use serde::{Deserialize, Serialize};

use crate::ScanReport;
use crate::error::{CoreError, Result};
use crate::key::{ItemKey, RootedPath};

mod edit;

pub use edit::SelectionEdit;

/// Pre-fill shortcuts. They only set `included`; the user edits after.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum Preset {
    /// Tick what Carrova suggests (recipe `default`, never sensitive/raw/cache).
    #[default]
    Suggested,
    /// Only non-sensitive config items under `user_config`/`home`.
    Minimal,
    /// Every movable, non-sensitive item incl. raw dirs (sensitive still need confirmation).
    Everything,
    /// Nothing ticked.
    None,
}

/// The user's choice for one item.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ItemChoice {
    /// Ticked.
    pub included: bool,
    /// Carrova suggests ticking it (display hint only).
    pub suggested: bool,
    /// Item is sensitive; including it requires `confirmed_sensitive`.
    pub sensitive: bool,
    /// User explicitly confirmed moving this sensitive item.
    pub confirmed_sensitive: bool,
    /// Recipe marks it unmovable (reason); including requires `force_unmovable`.
    pub unmovable: Option<String>,
    /// User overrode the unmovable mark (after seeing the reason).
    pub force_unmovable: bool,
    /// Paths/files under the item the user unticked (`"<root>:<rel>"`;
    /// a directory key excludes everything below it).
    pub excluded_paths: Vec<String>,
    /// Paths the item declares (`"<root>:<rel>"`), used to check exclusions
    /// and presets.
    #[serde(default)]
    pub paths: Vec<String>,
}

/// A problem that stops a selection (or one edit of it) from being used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SelectionIssue {
    /// The item concerned.
    pub item: ItemKey,
    /// What is wrong, user-presentable.
    pub problem: String,
}

impl std::fmt::Display for SelectionIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.item, self.problem)
    }
}

impl From<SelectionIssue> for CoreError {
    fn from(issue: SelectionIssue) -> Self {
        Self::Selection(issue.to_string())
    }
}

impl ItemChoice {
    /// Neither sensitive nor unmovable: app rows and presets may tick it.
    #[must_use]
    pub fn is_ordinary(&self) -> bool {
        !self.sensitive && self.unmovable.is_none()
    }

    /// May be ticked without an override: not unmovable, or forced.
    #[must_use]
    pub fn is_movable(&self) -> bool {
        self.unmovable.is_none() || self.force_unmovable
    }

    /// Some declared path lies under the cache root.
    #[must_use]
    pub fn is_cache(&self) -> bool {
        roots(&self.paths).any(|r| r == Some(SemanticRoot::Cache))
    }
}

/// Full user selection. Ordered map → deterministic output.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Selection {
    pub items: BTreeMap<ItemKey, ItemChoice>,
}

/// Root of every parseable path key in `paths`.
fn roots(paths: &[String]) -> impl Iterator<Item = Option<SemanticRoot>> + '_ {
    paths.iter().map(|p| RootedPath::parse(p).map(|p| p.root))
}

impl Selection {
    /// Build choices for every item in `scan` (detected apps + raw dirs),
    /// marking suggestions and applying `preset`.
    pub fn from_scan(scan: &ScanReport, preset: Preset) -> Self {
        let mut items = BTreeMap::new();
        for app in scan.apps.iter().filter(|a| a.detected) {
            for item in &app.items {
                let mut choice = ItemChoice {
                    sensitive: item.flags.sensitive,
                    unmovable: item.unmovable.clone(),
                    paths: item.paths.clone(),
                    ..ItemChoice::default()
                };
                choice.suggested =
                    item.flags.default_selected && choice.is_ordinary() && !choice.is_cache();
                items.insert(ItemKey::recipe(&app.id, &item.id), choice);
            }
        }
        for raw in &scan.unrecognised {
            let path = RootedPath::new(raw.root, &raw.rel);
            items.insert(
                ItemKey::raw(path),
                ItemChoice {
                    paths: vec![path.to_string()],
                    ..ItemChoice::default()
                },
            );
        }
        let mut sel = Self { items };
        sel.apply_preset(preset);
        sel
    }

    /// Re-apply a preset to `included` (keeps confirmations/exclusions).
    pub fn apply_preset(&mut self, preset: Preset) {
        for c in self.items.values_mut() {
            c.included = match preset {
                Preset::Suggested => c.suggested,
                Preset::Minimal => {
                    c.suggested
                        && !c.paths.is_empty()
                        && roots(&c.paths).all(|r| {
                            matches!(r, Some(SemanticRoot::UserConfig | SemanticRoot::Home))
                        })
                }
                Preset::Everything => c.is_ordinary(),
                Preset::None => false,
            };
        }
    }

    /// Problems that block capture (sensitive included but unconfirmed, …).
    #[must_use]
    pub fn issues(&self) -> Vec<SelectionIssue> {
        let mut out = Vec::new();
        for (key, c) in &self.items {
            let mut issue = |problem: String| {
                out.push(SelectionIssue {
                    item: key.clone(),
                    problem,
                });
            };
            if c.included && c.sensitive && !c.confirmed_sensitive {
                issue("sensitive item needs explicit confirmation".into());
            }
            if c.included
                && !c.force_unmovable
                && let Some(reason) = &c.unmovable
            {
                issue(format!("item is unmovable ({reason}); force it to include"));
            }
            for ex in &c.excluded_paths {
                match RootedPath::parse(ex) {
                    None => issue(format!("excluded path `{ex}` is not `<root>:<rel>`")),
                    Some(path) => {
                        let inside = c
                            .paths
                            .iter()
                            .filter_map(|p| RootedPath::parse(p))
                            .any(|base| path.is_within(base));
                        if !c.paths.is_empty() && !inside {
                            issue(format!("excluded path `{ex}` is not under this item"));
                        }
                    }
                }
            }
        }
        out
    }

    /// [`CoreError::Selection`] listing every [`Self::issues`], if there
    /// are any.
    pub fn check(&self) -> Result<()> {
        let issues = self.issues();
        if issues.is_empty() {
            return Ok(());
        }
        Err(refusal(&issues))
    }

    /// Keys of included items, in order.
    pub fn included(&self) -> impl Iterator<Item = &ItemKey> {
        self.items
            .iter()
            .filter(|(_, c)| c.included)
            .map(|(k, _)| k)
    }
}

/// [`CoreError::Selection`] listing `issues` (`item: problem`,
/// `; `-separated).
pub(crate) fn refusal(issues: &[SelectionIssue]) -> CoreError {
    CoreError::Selection(
        issues
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("; "),
    )
}
