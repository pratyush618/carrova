//! Receiver-side sensitivity and item selection (see the `plan` module
//! docs, "Receiver-side trust"). Decided from local knowledge only: the
//! archive flag can add, never remove, sensitivity.

use std::fmt;

use carrova_manifest::ManifestItem;
use carrova_recipe::Item as RecipeItem;

use super::Planner;
use crate::key::{ItemKey, RootedPath};
use crate::plan::{Blocker, recipe_sensitive};

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
            Self::Dangerous { why, at } => write!(f, "{why} ({at})"),
        }
    }
}

impl<'a> Planner<'a> {
    /// Classify every item (receiver-side sensitivity), then select:
    /// [`crate::PlanOptions::picks`] (explicit `items`, or by default every
    /// non-sensitive item plus the confirmed ones), minus what a foreign
    /// archive cannot restore here. A selected, unconfirmed sensitive item
    /// is a blocker.
    pub(super) fn select(&mut self) {
        for idx in 0..self.manifest.items.len() {
            let reasons = self.sensitivity(idx);
            if !reasons.is_empty() {
                self.sensitive
                    .insert(self.keys[idx].clone(), reasons.join("; "));
            }
        }
        let options = self.options;
        self.selected = (0..self.keys.len())
            .filter(|&i| {
                let key = &self.keys[i];
                options.picks(key, self.sensitive.get(key).map(String::as_str))
            })
            .collect();
        self.check_cross_os();
        let (keys, sensitive) = (&self.keys, &self.sensitive);
        for &i in &self.selected {
            if let Some(why) = sensitive.get(&keys[i])
                && !options.is_confirmed(&keys[i])
            {
                self.blockers.push(Blocker::UnconfirmedSensitive {
                    item: keys[i].clone(),
                    reason: why.clone(),
                });
            }
        }
    }

    /// Why item `idx` must be confirmed explicitly (empty = not sensitive).
    fn sensitivity(&self, idx: usize) -> Vec<String> {
        let item = &self.manifest.items[idx];
        let local = self.local_item(item);
        let mut reasons = Vec::new();
        if item.flags.sensitive {
            reasons.push(SensitiveReason::ArchiveFlag);
        }
        reasons.extend(self.recipe_reasons(&self.keys[idx], item, local));
        reasons.extend(self.dangerous_reasons(idx, local));
        reasons.iter().map(ToString::to_string).collect()
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
