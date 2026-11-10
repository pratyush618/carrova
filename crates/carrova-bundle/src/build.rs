//! Build side: recipe tree → bundle directory, and signing it.

use std::fs;
use std::path::Path;

use carrova_crypto::sign::{SignedKeyCert, SigningKey};
use carrova_recipe::{LoadedRecipe, load_str, load_tree, validate_tree};

use crate::{
    Asset, BlobKind, BlobRef, BundleError, BundleIndex, BundleVersion, DetectIndex, FORMAT,
    IndexSignature, RecipeEntry, Result, TierPolicy,
};

/// What [`build_bundle`] produced.
#[derive(Debug, Clone)]
pub struct BuildReport {
    /// The written (unsigned) index.
    pub index: BundleIndex,
    /// Number of files written, index included.
    pub files: usize,
}

/// Load and validate every recipe under `dir`; any load error or
/// validation issue fails with [`BundleError::InvalidRecipes`] listing all
/// of them.
pub fn load_checked_tree(dir: &Path) -> Result<Vec<LoadedRecipe>> {
    let tree = load_tree(dir)?;
    let mut problems: Vec<String> = tree.errors.iter().map(ToString::to_string).collect();
    problems.extend(validate_tree(&tree.recipes).iter().map(ToString::to_string));
    if problems.is_empty() {
        Ok(tree.recipes)
    } else {
        Err(BundleError::InvalidRecipes(problems))
    }
}

/// [`load_checked_tree`] plus the tree's [`TierPolicy`], checked to name
/// only recipes that exist.
pub(crate) fn load_tiered_tree(dir: &Path) -> Result<(Vec<LoadedRecipe>, TierPolicy)> {
    let loaded = load_checked_tree(dir)?;
    let tiers = TierPolicy::load(dir)?;
    tiers.check_known(loaded.iter().map(|l| l.recipe.id.as_str()))?;
    Ok((loaded, tiers))
}

/// Build an unsigned bundle of the recipe tree at `recipes_dir` (tiers from
/// its `TIERS.yaml`) into `out_dir`, which must be empty or absent.
pub fn build_bundle(
    recipes_dir: &Path,
    out_dir: &Path,
    version: BundleVersion,
    created: u64,
) -> Result<BuildReport> {
    let (loaded, tiers) = load_tiered_tree(recipes_dir)?;
    prepare_out_dir(out_dir)?;

    let mut recipes = Vec::with_capacity(loaded.len());
    for l in &loaded {
        let bytes = fs::read(&l.path).map_err(|e| BundleError::io(&l.path, e))?;
        check_blob_reparses(l, &bytes)?;
        let blob = write_blob(out_dir, BlobKind::Recipe, &bytes)?;
        recipes.push(RecipeEntry {
            id: l.recipe.id.clone(),
            category: l.recipe.category.clone(),
            tier: tiers.tier_of(&l.recipe.id),
            schema: l.recipe.schema,
            hash: blob.hash,
            size: blob.size,
        });
    }
    recipes.sort_by(|a, b| a.id.cmp(&b.id));

    let detect = DetectIndex::from_recipes(loaded.iter().map(|l| &l.recipe)).to_json()?;
    let index = BundleIndex {
        format: FORMAT,
        version,
        created,
        recipes,
        detect: write_blob(out_dir, BlobKind::Detect, &detect)?,
    };
    index.validate()?;
    let bytes = serde_json::to_vec_pretty(&index).map_err(BundleError::json("index"))?;
    write(out_dir, Asset::Index, &bytes)?;
    tracing::info!(%version, recipes = index.recipes.len(), "bundle built");
    Ok(BuildReport {
        files: index.recipes.len() + 2,
        index,
    })
}

/// Sign the bundle in `dir` with `signer` (certified by `cert`), writing
/// `index.sig.json`.
pub fn sign_bundle(dir: &Path, signer: &SigningKey, cert: SignedKeyCert) -> Result<IndexSignature> {
    let path = dir.join(Asset::INDEX_FILE);
    let index = fs::read(&path).map_err(|e| BundleError::io(&path, e))?;
    BundleIndex::parse(&index)?;
    let sig = IndexSignature::create(&index, signer, cert)?;
    write(dir, Asset::Signature, &sig.to_json()?)?;
    Ok(sig)
}

/// Clients parse blobs with `load_str`; make sure that yields exactly the
/// validated recipe.
fn check_blob_reparses(l: &LoadedRecipe, bytes: &[u8]) -> Result<()> {
    let text = std::str::from_utf8(bytes)
        .map_err(|e| BundleError::io(&l.path, std::io::Error::other(e)))?;
    if load_str(text, &l.path)? != l.recipe {
        return Err(BundleError::InvalidRecipes(vec![format!(
            "{}: changed while building",
            l.path.display()
        )]));
    }
    if bytes.len() as u64 > BlobKind::Recipe.max_size() {
        return Err(BundleError::InvalidRecipes(vec![format!(
            "{}: larger than {} bytes",
            l.path.display(),
            BlobKind::Recipe.max_size()
        )]));
    }
    Ok(())
}

fn prepare_out_dir(dir: &Path) -> Result<()> {
    fs::create_dir_all(dir).map_err(|e| BundleError::io(dir, e))?;
    let mut entries = fs::read_dir(dir).map_err(|e| BundleError::io(dir, e))?;
    if entries.next().is_some() {
        return Err(BundleError::OutputNotEmpty(dir.to_path_buf()));
    }
    Ok(())
}

fn write_blob(dir: &Path, kind: BlobKind, bytes: &[u8]) -> Result<BlobRef> {
    let blob = BlobRef::of(bytes);
    write(dir, Asset::Blob(kind, blob.hash), bytes)?;
    Ok(blob)
}

fn write(dir: &Path, asset: Asset, bytes: &[u8]) -> Result<()> {
    let path = dir.join(asset.file_name());
    fs::write(&path, bytes).map_err(|e| BundleError::io(path, e))
}
