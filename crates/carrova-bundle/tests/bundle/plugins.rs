//! Plugins in a bundle: built from `plugins/<id>/` next to the recipe
//! tree, shipped as signed content-addressed blobs, installed and loaded.

use std::fs;
use std::path::Path;

use carrova_bundle::{
    Asset, BlobKind, BundleError, Catalog, DirFetcher, UpdateOutcome, Updater, VerifyTime,
    build_bundle, open_bundle, plugins_dir,
};

use crate::support::{Keys, NOW, bundle, platform, store, write_tree};

const MANIFEST: &str = include_str!("../../../../plugins/path-rewrite/plugin.yaml");
const WASM: &[u8] = include_bytes!("../../../../plugins/path-rewrite/plugin.wasm");

/// `scratch/plugins/path-rewrite/`, next to the trees `bundle` writes.
fn write_plugin(scratch: &Path, manifest: &str) {
    let dir = scratch.join("plugins").join("path-rewrite");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("plugin.yaml"), manifest).unwrap();
    fs::write(dir.join("plugin.wasm"), WASM).unwrap();
}

#[test]
fn plugins_dir_is_the_recipe_trees_sibling() {
    assert_eq!(
        plugins_dir(Path::new("/src/recipes")),
        Path::new("/src/recipes/../plugins")
    );
}

#[test]
fn a_bundle_ships_plugins_as_signed_blobs() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    write_plugin(tmp.path(), MANIFEST);
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");

    let (signed, catalog) =
        open_bundle(&DirFetcher::new(&out), &keys.roots, VerifyTime::Now(NOW)).unwrap();
    let [entry] = signed.index().plugins.as_slice() else {
        panic!("one plugin expected: {:?}", signed.index().plugins);
    };
    assert_eq!(entry.id.as_str(), "path-rewrite");
    assert_eq!(entry.version, "1.0.0");
    let [plugin] = catalog.plugins.as_slice() else {
        panic!("one plugin expected");
    };
    assert_eq!(plugin.manifest.id.as_str(), "path-rewrite");
    assert_eq!(plugin.wasm, WASM);
    assert!(
        out.join(Asset::Blob(BlobKind::Plugin, entry.module.hash).file_name())
            .is_file()
    );
}

#[test]
fn a_tampered_plugin_blob_is_refused() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    write_plugin(tmp.path(), MANIFEST);
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");
    let (signed, _) =
        open_bundle(&DirFetcher::new(&out), &keys.roots, VerifyTime::Now(NOW)).unwrap();
    let module = signed.index().plugins[0].module;
    let blob = out.join(Asset::Blob(BlobKind::Plugin, module.hash).file_name());
    let mut bytes = fs::read(&blob).unwrap();
    bytes[10] ^= 1;
    fs::write(&blob, bytes).unwrap();

    let err = open_bundle(&DirFetcher::new(&out), &keys.roots, VerifyTime::Now(NOW)).unwrap_err();
    assert!(matches!(err, BundleError::BlobMismatch { .. }), "{err}");
}

#[test]
fn an_invalid_plugin_fails_the_build() {
    let tmp = tempfile::tempdir().unwrap();
    let tree = tmp.path().join("recipes");
    write_tree(&tree, "Alpha");
    write_plugin(
        tmp.path(),
        &MANIFEST.replace("id: path-rewrite", "id: other"),
    );
    let err = build_bundle(
        &tree,
        &tmp.path().join("out"),
        carrova_bundle::BundleVersion(1),
        NOW,
    )
    .unwrap_err();
    assert!(matches!(err, BundleError::InvalidPlugin { .. }), "{err}");
}

#[test]
fn the_installed_bundle_keeps_its_plugins() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    write_plugin(tmp.path(), MANIFEST);
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");

    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let updater = Updater::new(DirFetcher::new(&out), keys.roots.clone());
    let UpdateOutcome::Installed { stats, .. } = updater.update(&store, NOW).unwrap() else {
        panic!("expected install");
    };
    assert_eq!(
        stats.fetched, 5,
        "2 recipes + detect + plugin manifest + module"
    );
    assert_eq!(store.recipes().unwrap().plugins[0].wasm, WASM);
}

#[test]
fn a_dev_tree_loads_its_sibling_plugins_unsigned() {
    let tmp = tempfile::tempdir().unwrap();
    let tree = tmp.path().join("recipes");
    write_tree(&tree, "Alpha");
    write_plugin(tmp.path(), MANIFEST);
    let catalog = Catalog::from_tree(&tree).unwrap();
    assert_eq!(catalog.plugins.len(), 1);
    assert_eq!(catalog.plugins[0].manifest.id.as_str(), "path-rewrite");
}

#[test]
fn resolved_plugins_compile_for_the_engine() {
    let tmp = tempfile::tempdir().unwrap();
    let tree = tmp.path().join("recipes");
    write_tree(&tree, "Alpha");
    write_plugin(tmp.path(), MANIFEST);
    let keys = Keys::new();
    let resolved = carrova_bundle::resolve(&carrova_bundle::RecipeSources {
        dev_dir: Some(&tree),
        store: None,
        shipped_dir: None,
        shipped_pin: None,
        dev_fallback: None,
        roots: &keys.roots,
    })
    .unwrap();
    let (plugins, failed) = resolved.load_plugins();
    assert!(failed.is_empty(), "{failed:?}");
    assert_eq!(plugins[0].manifest().id.as_str(), "path-rewrite");
}
