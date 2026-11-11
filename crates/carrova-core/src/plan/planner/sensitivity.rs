//! Receiver-side sensitivity and item selection (see the `plan` module
//! docs, "Receiver-side trust"). Decided from local knowledge only: the
//! archive flag can add, never remove, sensitivity.

use std::fmt;

use carrova_manifest::ManifestItem;
use carrova_recipe::Item as RecipeItem;

use super::Planner;
use crate::key::{ItemKey, RootedPath};
use crate::plan::{Blocker, recipe_sensitive};
use crate::secrets::secret_in;

/// One reason an item needs explicit confirmation.
enum SensitiveReason<'a> {
    ArchiveFlag,
    Raw,
    UnknownRecipe(&'a str),
    UnknownItem {
        recipe: &'a str,
        item: &'a str,
    },
    RecipeFlag {
        recipe: &'a str,
        item: &'a str,
    },
    OutsideRecipe {
        recipe: &'a str,
        item: &'a str,
        example: String,
    },
    Dangerous {
        why: &'static str,
        at: String,
    },
    /// A recipe-less item landing in a well-known secret location.
    Secret {
        why: &'static str,
        at: String,
    },
}

impl fmt::Display for SensitiveReason<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const UNRECOGNISED: &str = "unrecognised data from the archive";
        match self {
            Self::ArchiveFlag => f.write_str("marked sensitive in the archive"),
            Self::Raw => f.write_str(UNRECOGNISED),
            Self::UnknownRecipe(r) => write!(f, "{UNRECOGNISED} (recipe `{r}` is not known here)"),
            Self::UnknownItem { recipe, item } => {
                write!(
                    f,
                    "{UNRECOGNISED} (recipe `{recipe}` has no item `{item}` here)"
                )
            }
            Self::RecipeFlag { recipe, item } => f.write_str(&recipe_sensitive(recipe, item)),
            Self::OutsideRecipe {
                recipe,
                item,
                example,
            } => write!(
                f,
                "contains paths outside recipe {recipe}/{item} (e.g. {example})"
            ),
            Self::Dangerous { why, at } | Self::Secret { why, at } => write!(f, "{why} ({at})"),
        }
    }
}

impl<'a> Planner<'a> {
    /// Classify every item (receiver-side sensitivity), then select:
    /// [`crate::PlanOptions::picks`] (explicit `items`, or by default every
    /// non-sensitive item plus the confirmed ones), minus what a foreign
    /// archive cannot restore here. A selected, unconfirmed sensitive item
    /// is a blocker. Same-OS items whose only reason (on their final
    /// targets) is "unrecognised data" become
    /// [`crate::plan::Plan::safe_picks`], which "Accept all"
    /// ([`crate::PlanOptions::accept_safe_picks`]) confirms.
    pub(super) fn select(&mut self) {
        let same_os = self.manifest.source.os == self.platform.os();
        for idx in 0..self.manifest.items.len() {
            let reasons = self.sensitivity(idx);
            let safe = same_os && matches!(reasons[..], [SensitiveReason::Raw]);
            let why = reasons
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ");
            if safe {
                self.safe_picks.push(self.keys[idx].clone());
            }
            if !why.is_empty() {
                self.sensitive.insert(self.keys[idx].clone(), why);
            }
        }
        let options = self.options;
        let safe = |key: &ItemKey| self.safe_picks.contains(key);
        self.selected = (0..self.keys.len())
            .filter(|&i| {
                let key = &self.keys[i];
                options.picks(key, self.sensitive.get(key).map(String::as_str), safe(key))
            })
            .collect();
        self.check_cross_os();
        let (keys, sensitive, safe) = (&self.keys, &self.sensitive, &self.safe_picks);
        for &i in &self.selected {
            if let Some(why) = sensitive.get(&keys[i])
                && !options.confirms(&keys[i], safe.contains(&keys[i]))
            {
                self.blockers.push(Blocker::UnconfirmedSensitive {
                    item: keys[i].clone(),
                    reason: why.clone(),
                });
            }
        }
    }

    /// Why item `idx` must be confirmed explicitly (empty = not sensitive).
    fn sensitivity(&self, idx: usize) -> Vec<SensitiveReason<'_>> {
        let item = &self.manifest.items[idx];
        let local = self.local_item(item);
        let mut reasons = Vec::new();
        if item.flags.sensitive {
            reasons.push(SensitiveReason::ArchiveFlag);
        }
        reasons.extend(self.recipe_reasons(&self.keys[idx], item, local));
        reasons.extend(self.secret_reason(idx));
        reasons.extend(self.dangerous_reasons(idx, local));
        reasons
    }

    /// For a recipe-less item: the first final target (after remap) in a
    /// well-known secret location, whatever the archive flag says.
    fn secret_reason(&self, idx: usize) -> Option<SensitiveReason<'static>> {
        let item = &self.manifest.items[idx];
        if item.recipe_id.is_some() {
            return None;
        }
        let key = &self.keys[idx];
        item.entries.iter().enumerate().find_map(|(i, e)| {
            let (root, rel) = self.remapped(key, i, e);
            secret_in(&rel).map(|why| SensitiveReason::Secret {
                why,
                at: RootedPath::new(root, &rel).to_string(),
            })
        })
    }

    /// The local recipe item an archive item claims to be, if known here.
    pub(super) fn local_item(&self, item: &ManifestItem) -> Option<&'a RecipeItem> {
        let recipe = self.recipe(item.recipe_id.as_deref()?)?;
        recipe.items.iter().find(|i| i.id == item.item_id)
    }

    /// Reasons from the local recipe: unknown here, flagged sensitive, or
    /// carrying paths (where they land before a user remap) outside the
    /// recipe item's declared paths.
    fn recipe_reasons<'i>(
        &'i self,
        key: &ItemKey,
        item: &'i ManifestItem,
        local: Option<&'i RecipeItem>,
    ) -> Vec<SensitiveReason<'i>> {
        let Some(recipe) = item.recipe_id.as_deref() else {
            return vec![SensitiveReason::Raw];
        };
        if self.recipe(recipe).is_none() {
            return vec![SensitiveReason::UnknownRecipe(recipe)];
        }
        let Some(li) = local else {
            let item = item.item_id.as_str();
            return vec![SensitiveReason::UnknownItem { recipe, item }];
        };
        let mut reasons = Vec::new();
        if li.flags.sensitive {
            reasons.push(SensitiveReason::RecipeFlag {
                recipe,
                item: &li.id,
            });
        }
        let local_paths = li.paths_for(self.platform.os());
        let outside = item
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| self.local_path(key, i, e))
            .find(|at| {
                !local_paths.iter().any(|p| {
                    self.roots
                        .within(*at, RootedPath::new(p.root, p.rel.as_str()))
                })
            });
        if let Some(at) = outside {
            reasons.push(SensitiveReason::OutsideRecipe {
                recipe,
                item: &li.id,
                example: at.to_string(),
            });
        }
        reasons
    }

    /// Dangerous final targets (after remap) and handler-changed files;
    /// one reason per kind of danger, at its first hit.
    fn dangerous_reasons(
        &self,
        idx: usize,
        local: Option<&RecipeItem>,
    ) -> Vec<SensitiveReason<'static>> {
        let item = &self.manifest.items[idx];
        let key = &self.keys[idx];
        let entries = item
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| self.remapped(key, i, e));
        let handler_paths = local
            .filter(|_| item.handler.is_some())
            .into_iter()
            .flat_map(|li| li.paths_for(self.platform.os()))
            .map(|p| (p.root, p.rel.as_str().to_owned()));
        let mut reasons: Vec<SensitiveReason<'static>> = Vec::new();
        for (root, rel) in entries.chain(handler_paths) {
            if let Some(d) = self.roots.dangerous(root, &rel)
                && !reasons
                    .iter()
                    .any(|r| matches!(r, SensitiveReason::Dangerous { why, .. } if *why == d.why))
            {
                reasons.push(SensitiveReason::Dangerous {
                    why: d.why,
                    at: RootedPath::new(root, &rel).to_string(),
                });
            }
        }
        reasons
    }
}
