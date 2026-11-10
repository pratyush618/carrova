//! Recipe fixture tests (`just fixtures [app]`).
//!
//! Every recipe under `recipes/<category>/<app>/` is checked on each OS it
//! supports (has `detect.<os>` rules for) against `fixture.<os>/`
//! (`fixture.macos/`, `fixture.windows/`), or `fixture/` when that is
//! absent. `fixture/` is the Linux fixture; a recipe whose files sit at
//! the same place on every OS (only `home` paths) needs no other.
//!
//! ```text
//! fixture/home/          fake home directory (dummy contents only)
//! fixture/expected.yaml  what detection and resolution must produce
//! ```
//!
//! The fake home lays the semantic roots out like the real OS
//! ([`fake_platform`]): Linux `.config`, `.local/share`, `.cache`;
//! macOS `Library/Application Support` (config + data), `Library/Caches`;
//! Windows `AppData/Roaming` (config), `AppData/Local` (data + cache);
//! `Documents` everywhere.
//!
//! `expected.yaml`:
//!
//! ```yaml
//! binaries: [git]          # names faked as installed
//! detected: true
//! process_names: [git]     # optional: Recipe::process_names(<fixture OS>), in order
//! items:                   # EVERY recipe item, unmovable ones with `files: []`
//!   config:
//!     handler: gpg-keyring # optional; must equal the item's handler (absent = none)
//!     files: ["home:.gitconfig", "user_config:git/config"]  # root:rel, files + symlinks, no dirs
//!     skipped: []          # optional, root:rel
//! not_captured:            # fixture files (relative to fixture/home) no item may capture
//!   - .cache/git/junk
//! ```
//!
//! Every file under `fixture/home` must be captured by some item or be
//! listed in `not_captured`, so excluded files are asserted explicitly.
//! Set `CARROVA_FIXTURE_FILTER=<recipe id>` to check a single recipe.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use carrova_platform::fake::FakePlatform;
use carrova_platform::{FileKind, OsKind, Platform, SemanticRoot};
use carrova_recipe::{LoadedRecipe, detect, load_tree, resolve_item, validate_tree};
use serde::Deserialize;
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Expected {
    #[serde(default)]
    binaries: Vec<String>,
    detected: bool,
    #[serde(default)]
    process_names: Option<Vec<String>>,
    items: BTreeMap<String, ExpectedItem>,
    #[serde(default)]
    not_captured: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExpectedItem {
    #[serde(default)]
    handler: Option<String>,
    files: Vec<String>,
    #[serde(default)]
    skipped: Vec<String>,
}

/// Markers that must never appear in a fixture (real key material).
const FORBIDDEN_MARKERS: &[&str] = &["PRIVATE KEY-----", "BEGIN PGP PRIVATE KEY"];

fn recipes_root() -> PathBuf {
    let raw = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../recipes");
    fs::canonicalize(&raw).unwrap_or(raw)
}

/// Load + validate the whole tree; `Err` lists every problem found.
fn load_all() -> Result<Vec<LoadedRecipe>, String> {
    let tree = load_tree(recipes_root()).map_err(|e| e.to_string())?;
    let mut problems: Vec<String> = tree.errors.iter().map(ToString::to_string).collect();
    problems.extend(validate_tree(&tree.recipes).iter().map(ToString::to_string));
    if tree.recipes.is_empty() {
        problems.push("no recipes found".into());
    }
    if problems.is_empty() {
        Ok(tree.recipes)
    } else {
        Err(format!(
            "recipe tree has problems:\n  {}",
            problems.join("\n  ")
        ))
    }
}

/// Recipes selected by `CARROVA_FIXTURE_FILTER` (empty/unset = all).
fn selected(all: Vec<LoadedRecipe>) -> Vec<LoadedRecipe> {
    let filter = std::env::var("CARROVA_FIXTURE_FILTER").unwrap_or_default();
    let filter = filter.trim();
    if filter.is_empty() {
        return all;
    }
    let ids: Vec<String> = all.iter().map(|l| l.recipe.id.clone()).collect();
    let picked: Vec<LoadedRecipe> = all.into_iter().filter(|l| l.recipe.id == filter).collect();
    assert!(
        !picked.is_empty(),
        "CARROVA_FIXTURE_FILTER={filter:?} matches no recipe (have: {ids:?})"
    );
    picked
}

fn diff(label: &str, want: &BTreeSet<String>, got: &BTreeSet<String>, errs: &mut Vec<String>) {
    let missing: Vec<&String> = want.difference(got).collect();
    let extra: Vec<&String> = got.difference(want).collect();
    if missing.is_empty() && extra.is_empty() {
        return;
    }
    let mut msg = format!("{label} mismatch");
    if !missing.is_empty() {
        let _ = write!(msg, "\n      expected but not resolved: {missing:?}");
    }
    if !extra.is_empty() {
        let _ = write!(msg, "\n      resolved but not expected: {extra:?}");
    }
    errs.push(msg);
}

/// Home-relative, `/`-joined path of every non-directory under `home`.
fn fixture_files(home: &Path, errs: &mut Vec<String>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for ent in WalkDir::new(home).follow_links(false) {
        let ent = match ent {
            Ok(e) => e,
            Err(e) => {
                errs.push(format!("walking fixture: {e}"));
                continue;
            }
        };
        if ent.file_type().is_dir() {
            continue;
        }
        let rel = ent.path().strip_prefix(home).unwrap_or(ent.path());
        let rel = rel.to_string_lossy().replace('\\', "/");
        if ent.file_type().is_file() {
            match fs::read(ent.path()) {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    for marker in FORBIDDEN_MARKERS {
                        if text.contains(marker) {
                            errs.push(format!(
                                "fixture file {rel} contains `{marker}` — fixtures must hold dummy data only"
                            ));
                        }
                    }
                }
                Err(e) => errs.push(format!("reading fixture file {rel}: {e}")),
            }
        }
        out.insert(rel);
    }
    out
}

/// `fixture.<os>/` (non-Linux), if the recipe ships one.
fn os_fixture(loaded: &LoadedRecipe, os: OsKind) -> Option<PathBuf> {
    let dir = loaded.dir().join(format!("fixture.{}", os.as_str()));
    (os != OsKind::Linux && dir.exists()).then_some(dir)
}

/// A fake `os` machine whose semantic roots sit under `home` where that OS
/// puts them.
fn fake_platform(home: &Path, os: OsKind) -> FakePlatform {
    let p = FakePlatform::with_home(home).with_os(os);
    let (config, data, cache) = match os {
        OsKind::Linux => return p,
        OsKind::MacOs => (
            "Library/Application Support",
            "Library/Application Support",
            "Library/Caches",
        ),
        OsKind::Windows => ("AppData/Roaming", "AppData/Local", "AppData/Local"),
    };
    p.root(SemanticRoot::UserConfig, home.join(config))
        .root(SemanticRoot::UserData, home.join(data))
        .root(SemanticRoot::Cache, home.join(cache))
}

/// Check one recipe against its `os` fixture. Returns failure messages.
fn check(loaded: &LoadedRecipe, os: OsKind) -> (Vec<String>, usize) {
    let mut errs = Vec::new();
    let recipe = &loaded.recipe;
    let fixture = os_fixture(loaded, os).unwrap_or_else(|| loaded.dir().join("fixture"));
    let home = fixture.join("home");
    let expected_path = fixture.join("expected.yaml");

    if !home.is_dir() {
        errs.push(format!("missing fixture home dir {}", home.display()));
        return (errs, 0);
    }
    let expected: Expected = match fs::read_to_string(&expected_path)
        .map_err(|e| e.to_string())
        .and_then(|t| serde_yaml_ng::from_str(&t).map_err(|e| e.to_string()))
    {
        Ok(e) => e,
        Err(e) => {
            errs.push(format!("{}: {e}", expected_path.display()));
            return (errs, 0);
        }
    };

    let mut platform = fake_platform(&home, os);
    for bin in &expected.binaries {
        platform = platform.binary(bin.as_str(), Path::new("/fake/bin").join(bin));
    }

    let detection = detect(recipe, &platform);
    if detection.detected != expected.detected {
        errs.push(format!(
            "detected = {}, expected {} (evidence: {:?})",
            detection.detected, expected.detected, detection.evidence
        ));
    }

    if let Some(want) = &expected.process_names {
        let got: Vec<&str> = recipe.process_names(os).collect();
        if got != *want {
            errs.push(format!("process_names = {got:?}, expected {want:?}"));
        }
    }

    let recipe_ids: BTreeSet<String> = recipe.items.iter().map(|i| i.id.clone()).collect();
    let expected_ids: BTreeSet<String> = expected.items.keys().cloned().collect();
    diff("item ids", &expected_ids, &recipe_ids, &mut errs);

    // Home-relative paths captured by any item.
    let mut captured: BTreeMap<String, String> = BTreeMap::new();
    let mut file_count = 0;
    for item in &recipe.items {
        let resolved = resolve_item(item, &platform);
        let got_files: BTreeSet<String> = resolved
            .entries
            .iter()
            .filter(|e| e.kind != FileKind::Dir)
            .map(|e| format!("{}:{}", e.root, e.rel))
            .collect();
        let got_skipped: BTreeSet<String> = resolved
            .skipped
            .iter()
            .map(|s| format!("{}:{}", s.root, s.rel))
            .collect();
        file_count += got_files.len();

        for e in resolved.entries.iter().filter(|e| e.kind != FileKind::Dir) {
            let Ok(root) = platform.resolve_root(e.root) else {
                continue;
            };
            let abs = root.join(&e.rel);
            if let Ok(rel) = abs.strip_prefix(&home) {
                captured.insert(rel.to_string_lossy().replace('\\', "/"), item.id.clone());
            }
        }

        let Some(want) = expected.items.get(&item.id) else {
            continue; // reported by the id diff
        };
        let got_handler = item.handler.as_ref().map(ToString::to_string);
        if got_handler != want.handler {
            errs.push(format!(
                "item `{}` handler = {got_handler:?}, expected {:?}",
                item.id, want.handler
            ));
        }
        let want_files: BTreeSet<String> = want.files.iter().cloned().collect();
        let want_skipped: BTreeSet<String> = want.skipped.iter().cloned().collect();
        diff(
            &format!("item `{}` files", item.id),
            &want_files,
            &got_files,
            &mut errs,
        );
        diff(
            &format!("item `{}` skipped", item.id),
            &want_skipped,
            &got_skipped,
            &mut errs,
        );
        if !item.is_movable() && !got_files.is_empty() {
            errs.push(format!("unmovable item `{}` resolved files", item.id));
        }
    }

    let on_disk = fixture_files(&home, &mut errs);
    let not_captured: BTreeSet<String> = expected.not_captured.iter().cloned().collect();
    for rel in &not_captured {
        if !on_disk.contains(rel) {
            errs.push(format!(
                "not_captured entry `{rel}` does not exist in fixture"
            ));
        }
        if let Some(item) = captured.get(rel) {
            errs.push(format!(
                "`{rel}` must not be captured, but item `{item}` captures it"
            ));
        }
    }
    for rel in &on_disk {
        if !captured.contains_key(rel) && !not_captured.contains(rel) {
            errs.push(format!(
                "fixture file `{rel}` is captured by no item and not listed in not_captured"
            ));
        }
    }
    (errs, file_count)
}

#[test]
fn recipe_fixtures() {
    let recipes = selected(load_all().unwrap());
    let mut failures: Vec<String> = Vec::new();
    for loaded in &recipes {
        for os in OsKind::ALL {
            let id = format!("{} ({})", loaded.recipe.id, os.as_str());
            let errs_files = if loaded.recipe.supports(os) {
                Some(check(loaded, os))
            } else {
                os_fixture(loaded, os).map(|stale| {
                    let msg = format!(
                        "{} exists but the recipe has no detect.{} rules",
                        stale.display(),
                        os.as_str()
                    );
                    (vec![msg], 0)
                })
            };
            let Some((errs, files)) = errs_files else {
                continue;
            };
            if errs.is_empty() {
                println!(
                    "ok    {id:<28} {} items, {files} files",
                    loaded.recipe.items.len()
                );
            } else {
                println!("FAIL  {id:<28} {} problem(s)", errs.len());
                failures.push(format!("{id}:\n    {}", errs.join("\n    ")));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} recipe fixture(s) failed:\n  {}",
        failures.len(),
        recipes.len(),
        failures.join("\n  ")
    );
}

/// An empty home with nothing installed must detect nothing, on any OS.
#[test]
fn empty_home_detects_nothing() {
    let recipes = selected(load_all().unwrap());
    let home = tempfile::tempdir().expect("tempdir");
    let hits: Vec<String> = OsKind::ALL
        .into_iter()
        .flat_map(|os| {
            let platform = fake_platform(home.path(), os);
            recipes
                .iter()
                .filter_map(|l| {
                    let d = detect(&l.recipe, &platform);
                    d.detected
                        .then(|| format!("{} ({os}): {:?}", l.recipe.id, d.evidence))
                })
                .collect::<Vec<_>>()
        })
        .collect();
    assert!(
        hits.is_empty(),
        "recipes detected on an empty home:\n  {}",
        hits.join("\n  ")
    );
}
