//! What to export or send (`--preset`, `--include`, …): shared by
//! `carrova export` and `carrova send`, turned into engine
//! [`SelectionEdit`]s.

use carrova_core::ItemKey;
use carrova_core::select::{Preset, Selection, SelectionEdit};
use clap::{Args, ValueEnum};

use crate::args::{PathExclusion, parse_exclude_path};

/// Selection pre-fill (`--preset`); every choice stays editable.
#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum PresetArg {
    /// What Carrova suggests (never sensitive, raw or cache data).
    Suggested,
    /// Only non-sensitive config under user_config/home.
    Minimal,
    /// Every movable non-sensitive item, incl. unrecognised dirs.
    Everything,
    /// Nothing; add items with --include.
    None,
}

impl From<PresetArg> for Preset {
    fn from(p: PresetArg) -> Self {
        match p {
            PresetArg::Suggested => Self::Suggested,
            PresetArg::Minimal => Self::Minimal,
            PresetArg::Everything => Self::Everything,
            PresetArg::None => Self::None,
        }
    }
}

/// What to export or send: shared by `carrova export` and `carrova send`.
#[derive(Debug, Args)]
pub struct SelectionArgs {
    /// Pre-fill the selection; edit it with --include/--exclude.
    #[arg(long, value_enum, default_value = "suggested")]
    preset: PresetArg,
    /// Tick an item: `<recipe>/<item>` or `raw:<root>/<rel>` (see `carrova scan`).
    #[arg(long, value_name = "KEY")]
    include: Vec<ItemKey>,
    /// Untick an item.
    #[arg(long, value_name = "KEY")]
    exclude: Vec<ItemKey>,
    /// Untick one path or file inside an item: `ITEM=ROOT:REL`.
    #[arg(long, value_name = "ITEM=ROOT:REL", value_parser = parse_exclude_path)]
    exclude_path: Vec<PathExclusion>,
    /// Explicitly confirm exporting a sensitive item (keys, tokens).
    #[arg(long, value_name = "ITEM")]
    confirm_sensitive: Vec<ItemKey>,
    /// Include an item the recipe marks unmovable (its reason is shown).
    #[arg(long, value_name = "ITEM")]
    force_unmovable: Vec<ItemKey>,
}

impl SelectionArgs {
    /// The pre-fill chosen with `--preset`.
    pub fn preset(&self) -> Preset {
        self.preset.into()
    }

    /// The user's edits as engine [`SelectionEdit`]s, in flag order:
    /// `--include`, `--exclude`, `--exclude-path`, `--confirm-sensitive`,
    /// `--force-unmovable`.
    pub fn edits(&self) -> impl Iterator<Item = SelectionEdit> + '_ {
        let tick = |included| {
            move |item: &ItemKey| SelectionEdit::Item {
                item: item.clone(),
                included,
            }
        };
        self.include
            .iter()
            .map(tick(true))
            .chain(self.exclude.iter().map(tick(false)))
            .chain(self.exclude_path.iter().map(|e| SelectionEdit::Path {
                item: e.item.clone(),
                path: e.path.clone(),
                included: false,
            }))
            .chain(
                self.confirm_sensitive
                    .iter()
                    .map(|item| SelectionEdit::ConfirmSensitive {
                        item: item.clone(),
                        confirmed: true,
                    }),
            )
            .chain(
                self.force_unmovable
                    .iter()
                    .map(|item| SelectionEdit::ForceUnmovable { item: item.clone() }),
            )
    }

    /// Apply the user's edits ([`Self::edits`]) to `sel`.
    pub fn edit(&self, sel: &mut Selection) -> carrova_core::Result<()> {
        Ok(sel.apply_all(self.edits())?)
    }
}
