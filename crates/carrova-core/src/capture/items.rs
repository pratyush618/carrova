//! Turning a selection's included items into something capturable
//! ([`Planned`]), and the running-app warning.

use carrova_manifest::SourcePath;
use carrova_manifest::{ItemFlags, ManifestItem};
use carrova_platform::{OsKind, Platform, ProcessInfo};
use carrova_recipe::{Flags, Item, PathRef, Portability, Recipe, RelPath};

use crate::checklist::Skipped;
use crate::key::{ItemKey, ItemTarget, RootedPath};
use crate::select::{ItemChoice, Selection};
use crate::session::{Event, EventSink};

/// One included item, ready to capture.
pub(super) struct Planned<'a> {
    pub(super) key: &'a ItemKey,
    pub(super) recipe: Option<&'a Recipe>,
    /// Item to resolve (synthetic for raw dirs; unmovable mark cleared when
    /// the user forced it).
    pub(super) item: Item,
    /// Paths the user unticked.
    pub(super) excluded: Vec<RootedPath<'a>>,
    /// A raw item the user did not confirm as sensitive: what lies in a
    /// secret location now was not there when it was judged, so it stays
    /// behind ([`super::walk::hold_back_secrets`]).
    pub(super) unconfirmed_raw: bool,
    /// The manifest item to fill (entries or artifacts still empty).
    pub(super) manifest_item: ManifestItem,
}

/// Every included item of `selection`, planned for capture on `os`; items
/// that cannot be go to the returned skip list with a reason.
pub(super) fn plan_items<'a>(
    selection: &'a Selection,
    recipes: &'a [Recipe],
    os: OsKind,
) -> (Vec<Planned<'a>>, Vec<Skipped>) {
    let mut planned = Vec::new();
    let mut failed = Vec::new();
    for (key, choice) in selection.items.iter().filter(|(_, c)| c.included) {
        match plan_item(key, choice, recipes, os) {
            Ok(p) => planned.push(p),
            Err(reason) => failed.push(Skipped {
                what: key.to_string(),
                reason,
            }),
        }
    }
    (planned, failed)
}

/// Turn one included selection entry into something capturable, or a
/// human-readable reason it cannot be.
fn plan_item<'a>(
    key: &'a ItemKey,
    choice: &'a ItemChoice,
    recipes: &'a [Recipe],
    os: OsKind,
) -> Result<Planned<'a>, String> {
    let excluded = choice
        .excluded_paths
        .iter()
        .filter_map(|p| RootedPath::parse(p))
        .collect();
    match key.target() {
        Some(ItemTarget::Raw(path)) => plan_raw_item(key, path, choice, excluded),
        Some(ItemTarget::Recipe { recipe, item }) => {
            plan_recipe_item(key, (recipe, item), choice, recipes, excluded, os)
        }
        None if key.is_raw() => Err(format!("malformed raw item key `{key}`")),
        None => Err(format!("malformed item key `{key}`")),
    }
}

/// A raw item (an unrecognised directory or a Manual pick) as a
/// synthetic one-path item whose id is [`RootedPath::raw_item_id`],
/// sensitive when its choice is (same-OS, like every raw item).
fn plan_raw_item<'a>(
    key: &'a ItemKey,
    path: RootedPath<'a>,
    choice: &ItemChoice,
    excluded: Vec<RootedPath<'a>>,
) -> Result<Planned<'a>, String> {
    let RootedPath { root, rel } = path;
    let rel_path = RelPath::new_raw(rel).map_err(|e| format!("invalid raw path: {e}"))?;
    let id = path.raw_item_id();
    Ok(Planned {
        key,
        recipe: None,
        item: Item {
            id: id.clone(),
            label: rel.to_owned(),
            paths: vec![PathRef {
                root,
                rel: rel_path,
            }],
            exclude: Vec::new(),
            flags: Flags::default(),
            unmovable: None,
            handler: None,
            os: None,
        },
        excluded,
        unconfirmed_raw: !choice.confirmed_sensitive,
        manifest_item: ManifestItem {
            recipe_id: None,
            item_id: id,
            label: rel.to_owned(),
            flags: ItemFlags {
                sensitive: choice.sensitive,
                ..ItemFlags::default()
            },
            entries: Vec::new(),
            handler: None,
            artifacts: Vec::new(),
            source_paths: Vec::new(),
        },
    })
}

/// Item `item_id` of loaded recipe `recipe_id` (selected as `key`), with
/// its paths on `os`.
fn plan_recipe_item<'a>(
    key: &'a ItemKey,
    (recipe_id, item_id): (&str, &str),
    choice: &ItemChoice,
    recipes: &'a [Recipe],
    excluded: Vec<RootedPath<'a>>,
    os: OsKind,
) -> Result<Planned<'a>, String> {
    let recipe = recipes
        .iter()
        .find(|r| r.id == recipe_id)
        .ok_or_else(|| format!("recipe `{recipe_id}` is not loaded"))?;
    let mut item = recipe
        .items
        .iter()
        .find(|i| i.id == item_id)
        .cloned()
        .ok_or_else(|| format!("recipe `{recipe_id}` has no item `{item_id}`"))?;
    if item.unmovable.is_some() && choice.force_unmovable {
        // The user saw the reason and forced it.
        item.unmovable = None;
    }
    let paths = item.paths_for(os);
    if paths.is_empty() && item.builtin_handler().is_none() {
        return Err("item declares no paths".into());
    }
    let source_paths = paths
        .iter()
        .map(source_path)
        .collect::<Result<Vec<_>, _>>()?;
    let manifest_item = ManifestItem {
        recipe_id: Some(recipe.id.clone()),
        item_id: item.id.clone(),
        label: item.label.clone(),
        flags: ItemFlags {
            sensitive: item.flags.sensitive,
            requires_closed: item.flags.requires_closed,
            portability: match item.flags.portability {
                Portability::SameOs => carrova_manifest::Portability::SameOs,
                Portability::CrossOs => carrova_manifest::Portability::CrossOs,
            },
        },
        entries: Vec::new(),
        handler: None,
        artifacts: Vec::new(),
        source_paths,
    };
    Ok(Planned {
        key,
        recipe: Some(recipe),
        item,
        excluded,
        unconfirmed_raw: false,
        manifest_item,
    })
}

/// A recipe path as the manifest records it (the manifest's own, stricter
/// relative-path rules apply).
fn source_path(p: &PathRef) -> Result<SourcePath, String> {
    let rel = carrova_manifest::RelPath::new(p.rel.as_str())
        .map_err(|e| format!("recipe path {p} cannot be recorded: {e}"))?;
    Ok(SourcePath { root: p.root, rel })
}

/// Warn (never block: export only reads) for included `requires_closed`
/// items whose app is running.
pub(super) fn warn_running(platform: &dyn Platform, planned: &[Planned<'_>], sink: &dyn EventSink) {
    let needs: Vec<(&Planned<'_>, &Recipe)> = planned
        .iter()
        .filter(|p| p.item.flags.requires_closed)
        .filter_map(|p| p.recipe.map(|r| (p, r)))
        .collect();
    if needs.is_empty() {
        return;
    }
    let procs: Vec<ProcessInfo> = match platform.running_processes() {
        Ok(p) => p,
        Err(e) => {
            sink.emit(Event::Warning {
                message: format!("cannot check whether apps are running: {e}"),
            });
            return;
        }
    };
    for (p, recipe) in needs {
        let running = recipe
            .process_names(platform.os())
            .any(|n| procs.iter().any(|pr| pr.matches_binary(n)));
        if running {
            sink.emit(Event::Warning {
                message: format!(
                    "{} is running; {} should be captured with it closed (captured anyway, data may be inconsistent)",
                    recipe.name, p.key
                ),
            });
        }
    }
}
