//! Shared fixtures: a tiny recipe tree, a signing chain, bundle dirs and a
//! fake platform for the store.

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};

use carrova_bundle::{
    Asset, BundleIndex, BundleStore, BundleVersion, DirFetcher, FetchError, Fetcher,
    IndexSignature, build_bundle, sign_bundle,
};
use carrova_crypto::sign::{KeyCert, KeyRole, SignedKeyCert, SigningKey, TrustRoots};
use carrova_platform::fake::FakePlatform;

pub const NOW: u64 = 1_800_000_000;
const DAY: u64 = 86_400;

/// Root + certified signer.
#[derive(Debug)]
pub struct Keys {
    pub roots: TrustRoots,
    pub signer: SigningKey,
    pub cert: SignedKeyCert,
}

impl Default for Keys {
    fn default() -> Self {
        Self::new()
    }
}

impl Keys {
    pub fn new() -> Self {
        let root = SigningKey::generate().unwrap();
        let signer = SigningKey::generate().unwrap();
        let cert = KeyCert {
            key: signer.verifying_key(),
            role: KeyRole::BundleSigner,
            not_before: NOW - DAY,
            not_after: NOW + 30 * DAY,
        }
        .sign(&root)
        .unwrap();
        Self {
            roots: TrustRoots::new([root.verifying_key()]).unwrap(),
            signer,
            cert,
        }
    }

    /// Sign `dir/index.json` byte for byte, without the builder's checks
    /// (to forge malformed but validly signed indexes).
    pub fn sign_raw(&self, dir: &Path) {
        let index = fs::read(dir.join(Asset::INDEX_FILE)).unwrap();
        let sig = IndexSignature::create(&index, &self.signer, self.cert).unwrap();
        fs::write(dir.join(Asset::SIGNATURE_FILE), sig.to_json().unwrap()).unwrap();
    }

    /// Re-sign `dir/index.json` as it is now.
    pub fn resign(&self, dir: &Path) {
        sign_bundle(dir, &self.signer, self.cert).unwrap();
    }
}

pub fn recipe(id: &str, category: &str, description: &str) -> String {
    format!(
        "schema: 1\nid: {id}\nname: {id}\ncategory: {category}\ndescription: {description}\n\
         detect:\n  linux:\n    - binary: {id}\n\
         items:\n  - id: config\n    label: Config\n    paths: [{{ root: user_config, rel: {id} }}]\n"
    )
}

/// Recipe tree with `alpha` (verified) and `beta` (community); `alpha`'s
/// description is `alpha_desc` so tests can change exactly one blob.
pub fn write_tree(root: &Path, alpha_desc: &str) {
    for (id, cat, desc) in [("alpha", "tools", alpha_desc), ("beta", "shells", "Beta")] {
        let dir = root.join(cat).join(id);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("recipe.yaml"), recipe(id, cat, desc)).unwrap();
    }
    fs::write(root.join("TIERS.yaml"), "verified: [alpha]\n").unwrap();
}

/// Build and sign bundle `version` of a fresh tree into `out`.
pub fn bundle(keys: &Keys, scratch: &Path, out: &Path, version: u64, alpha_desc: &str) {
    bundle_at(keys, scratch, out, version, alpha_desc, NOW);
}

/// [`bundle`] with an explicit creation time.
pub fn bundle_at(
    keys: &Keys,
    scratch: &Path,
    out: &Path,
    version: u64,
    alpha_desc: &str,
    created: u64,
) {
    let tree = scratch.join(format!("tree-{version}-{}-{created}", alpha_desc.len()));
    write_tree(&tree, alpha_desc);
    build_bundle(&tree, out, BundleVersion(version), created).unwrap();
    keys.resign(out);
}

/// Rewrite `index.json` through `edit` and re-sign it.
pub fn edit_index(keys: &Keys, dir: &Path, edit: impl FnOnce(&mut BundleIndex)) {
    let path = dir.join(Asset::INDEX_FILE);
    let mut index: BundleIndex = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    edit(&mut index);
    fs::write(&path, serde_json::to_vec_pretty(&index).unwrap()).unwrap();
    keys.resign(dir);
}

/// Fake platform whose user-data root lies in `home`.
pub fn platform(home: &Path) -> FakePlatform {
    FakePlatform::with_home(home)
}

/// The store on `p`, trusting `roots`.
pub fn store<'p>(p: &'p FakePlatform, roots: &TrustRoots) -> BundleStore<'p> {
    BundleStore::open(p, roots.clone()).unwrap()
}

/// A [`DirFetcher`] that records every asset requested.
#[derive(Debug)]
pub struct CountingFetcher {
    inner: DirFetcher,
    pub requests: RefCell<Vec<String>>,
}

impl CountingFetcher {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self {
            inner: DirFetcher::new(dir),
            requests: RefCell::default(),
        }
    }

    /// Requested blob names (index and signature excluded).
    pub fn blobs(&self) -> Vec<String> {
        self.requests
            .borrow()
            .iter()
            .filter(|n| !n.starts_with("index"))
            .cloned()
            .collect()
    }
}

impl Fetcher for CountingFetcher {
    fn get(&self, asset: &Asset, limit: u64) -> Result<Vec<u8>, FetchError> {
        self.requests.borrow_mut().push(asset.file_name());
        self.inner.get(asset, limit)
    }
}
