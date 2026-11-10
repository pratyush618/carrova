//! Recipe source resolution and bundle update as the CLI wires them, with
//! bundles signed by a test root (never the release roots).

mod reset;

use std::fs;
use std::path::{Path, PathBuf};

use anyhow::Result;
use carrova_bundle::{
    BundleStore, BundleVersion, CheckAttempt, DirFetcher, RecipeSource, RecipeSources,
    ResolvedRecipes, TrustRoots, UpdateOutcome, Updater, build_bundle, sign_bundle,
};
use carrova_crypto::sign::{KeyCert, KeyRole, SigningKey};
use carrova_platform::fake::FakePlatform;
use tempfile::TempDir;

use super::RecipeLocations;
use super::error::{Action, explain};
use super::source::{shipped_dir_for, unverified_warning};
use super::update::{checked, outcome};
use crate::render::recipes::{check_outcome, update_outcome};

pub(super) const NOW: u64 = 1_800_000_000;
const DAY: u64 = 86_400;
pub(super) const URL: &str = "https://example.invalid/recipes-latest/";

/// [`carrova_bundle::resolve`] with its errors explained, as `load` does.
pub(super) fn resolve_recipes(sources: &RecipeSources<'_>) -> Result<ResolvedRecipes> {
    carrova_bundle::resolve(sources).map_err(|e| explain(e, Action::Load))
}

/// A test root and a bundle signer it certifies.
pub(super) struct Signer {
    pub(super) roots: TrustRoots,
    key: SigningKey,
    cert: carrova_crypto::sign::SignedKeyCert,
}

impl Signer {
    pub(super) fn new() -> Result<Self> {
        let root = SigningKey::generate()?;
        let key = SigningKey::generate()?;
        let cert = KeyCert {
            key: key.verifying_key(),
            role: KeyRole::BundleSigner,
            not_before: NOW - DAY,
            not_after: NOW + 30 * DAY,
        }
        .sign(&root)?;
        Ok(Self {
            roots: TrustRoots::new([root.verifying_key()])?,
            key,
            cert,
        })
    }

    /// Build and sign bundle `version` of a fresh recipe tree into `out`.
    fn bundle(&self, scratch: &Path, out: &Path, version: u64) -> Result<()> {
        let tree = scratch.join(format!("tree-{version}"));
        write_tree(&tree, &format!("v{version}"))?;
        build_bundle(&tree, out, BundleVersion(version), NOW)?;
        sign_bundle(out, &self.key, self.cert)?;
        Ok(())
    }
}

/// Recipe tree with `alpha` (verified) and `beta` (community).
fn write_tree(root: &Path, description: &str) -> Result<()> {
    for (id, category) in [("alpha", "tools"), ("beta", "shells")] {
        let dir = root.join(category).join(id);
        fs::create_dir_all(&dir)?;
        fs::write(
            dir.join("recipe.yaml"),
            format!(
                "schema: 1\nid: {id}\nname: {id}\ncategory: {category}\n\
                 description: {description}\n\
                 detect:\n  linux:\n    - binary: {id}\n\
                 items:\n  - id: config\n    label: Config\n    \
                 paths: [{{ root: user_config, rel: {id} }}]\n"
            ),
        )?;
    }
    fs::write(root.join("TIERS.yaml"), "verified: [alpha]\n")?;
    Ok(())
}

/// Temp dir with a dev tree, a shipped bundle (v3) and a fake home.
pub(super) struct Fixture {
    tmp: TempDir,
    pub(super) signer: Signer,
    pub(super) platform: FakePlatform,
}

impl Fixture {
    pub(super) fn new() -> Result<Self> {
        let tmp = TempDir::new()?;
        let signer = Signer::new()?;
        write_tree(&tmp.path().join("dev"), "dev")?;
        write_tree(&tmp.path().join("checkout"), "checkout")?;
        signer.bundle(tmp.path(), &tmp.path().join("shipped"), 3)?;
        let platform = FakePlatform::with_home(tmp.path().join("home"));
        Ok(Self {
            tmp,
            signer,
            platform,
        })
    }

    pub(super) fn path(&self, name: &str) -> PathBuf {
        self.tmp.path().join(name)
    }

    pub(super) fn store(&self) -> Result<BundleStore<'_>> {
        Ok(BundleStore::open(
            &self.platform,
            self.signer.roots.clone(),
        )?)
    }

    /// Publish bundle `version` as a local "release" directory.
    pub(super) fn release(&self, version: u64) -> Result<PathBuf> {
        let release = self.path(&format!("release-{version}"));
        self.signer.bundle(self.tmp.path(), &release, version)?;
        Ok(release)
    }

    /// `carrova recipes update` from the local `release`.
    pub(super) fn update(&self, release: &Path) -> Result<UpdateOutcome> {
        outcome(
            carrova_bundle::update_with(
                DirFetcher::new(release),
                self.signer.roots.clone(),
                &self.store()?,
                NOW,
            ),
            URL,
        )
    }

    /// Publish and install bundle `version`.
    pub(super) fn install(&self, version: u64) -> Result<UpdateOutcome> {
        self.update(&self.release(version)?)
    }

    pub(super) fn resolve(&self, locations: &RecipeLocations) -> Result<RecipeSource> {
        let store = self.store()?;
        Ok(resolve_recipes(&locations.sources(Some(&store), &self.signer.roots))?.source)
    }
}

#[test]
fn dev_override_wins_and_is_flagged_unverified() -> Result<()> {
    let f = Fixture::new()?;
    f.install(5)?;
    let locations = RecipeLocations {
        dev_override: Some(f.path("dev")),
        shipped_dir: Some(f.path("shipped")),
        dev_fallback: Some(f.path("checkout")),
    };
    let source = f.resolve(&locations)?;
    assert_eq!(source, RecipeSource::DevDir(f.path("dev")));
    let warning = unverified_warning(&source).unwrap_or_default();
    assert!(warning.contains("unverified recipes"), "{warning}");
    Ok(())
}

#[test]
fn installed_bundle_beats_shipped_and_checkout() -> Result<()> {
    let f = Fixture::new()?;
    let locations = RecipeLocations {
        dev_override: None,
        shipped_dir: Some(f.path("shipped")),
        dev_fallback: Some(f.path("checkout")),
    };
    assert!(matches!(
        f.resolve(&locations)?,
        RecipeSource::Shipped {
            version: BundleVersion(3),
            ..
        }
    ));
    f.install(5)?;
    let source = f.resolve(&locations)?;
    assert_eq!(source, RecipeSource::Installed(BundleVersion(5)));
    assert_eq!(unverified_warning(&source), None);
    Ok(())
}

#[test]
fn checkout_fallback_is_used_only_without_any_bundle() -> Result<()> {
    let f = Fixture::new()?;
    let locations = RecipeLocations {
        dev_fallback: Some(f.path("checkout")),
        ..RecipeLocations::default()
    };
    let source = f.resolve(&locations)?;
    assert_eq!(source, RecipeSource::DevDir(f.path("checkout")));
    assert!(unverified_warning(&source).is_some());
    Ok(())
}

#[test]
fn no_bundle_without_checkout_fallback_says_run_recipes_update() -> Result<()> {
    let f = Fixture::new()?;
    let Err(err) = f.resolve(&RecipeLocations::default()) else {
        anyhow::bail!("resolved without any source");
    };
    let msg = err.to_string();
    assert!(msg.contains("Run `carrova recipes update`"), "{msg}");
    Ok(())
}

#[test]
fn shipped_bundle_lives_in_share_next_to_bin() {
    assert_eq!(
        shipped_dir_for(Path::new("/opt/carrova/bin/carrova")),
        Some(PathBuf::from("/opt/carrova/share/carrova/recipes"))
    );
    assert_eq!(shipped_dir_for(Path::new("carrova")), None);
}

#[test]
fn update_installs_a_newer_bundle_then_reports_up_to_date() -> Result<()> {
    let f = Fixture::new()?;
    let mut text = Vec::new();
    let release = f.release(4)?;
    update_outcome(&mut text, &f.update(&release)?)?;
    update_outcome(&mut text, &f.update(&release)?)?;
    let text = String::from_utf8(text)?;
    assert!(
        text.starts_with("Installed recipe bundle v4 (first install): downloaded 3 files"),
        "{text}"
    );
    assert!(
        text.ends_with("Recipes are already up to date (v4).\n"),
        "{text}"
    );
    Ok(())
}

#[test]
fn update_during_tag_repoint_suggests_trying_again() -> Result<()> {
    let f = Fixture::new()?;
    let empty = f.path("empty-release");
    fs::create_dir_all(&empty)?;
    let Err(err) = outcome(
        carrova_bundle::update_with(
            DirFetcher::new(empty),
            f.signer.roots.clone(),
            &f.store()?,
            NOW,
        ),
        URL,
    ) else {
        anyhow::bail!("updated from an empty release");
    };
    let msg = err.to_string();
    assert!(msg.contains("try again in a minute"), "{msg}");
    assert!(msg.contains(URL), "{msg}");
    Ok(())
}

#[test]
fn update_refuses_a_bundle_from_an_untrusted_root() -> Result<()> {
    let f = Fixture::new()?;
    let Err(err) = outcome(
        carrova_bundle::update_with(
            DirFetcher::new(f.path("shipped")),
            Signer::new()?.roots,
            &f.store()?,
            NOW,
        ),
        URL,
    ) else {
        anyhow::bail!("installed a bundle from an untrusted root");
    };
    assert!(
        err.to_string().contains("not signed by a trusted key"),
        "{err}"
    );
    assert_eq!(f.store()?.installed()?, None);
    Ok(())
}

#[test]
fn update_check_reports_available_then_up_to_date_and_installs_nothing() -> Result<()> {
    let f = Fixture::new()?;
    let check = |release: &Path| -> Result<String> {
        let attempt = Updater::new(DirFetcher::new(release), f.signer.roots.clone())
            .check(&f.store()?, NOW)
            .map(CheckAttempt::Done);
        let mut text = Vec::new();
        check_outcome(&mut text, &checked(attempt, URL)?)?;
        Ok(String::from_utf8(text)?)
    };
    let release = f.release(4)?;
    assert_eq!(
        check(&release)?,
        "Recipe bundle v4 is available; `carrova recipes update` installs it.\n"
    );
    assert!(
        f.store()?.installed()?.is_none(),
        "check installed a bundle"
    );
    f.update(&release)?;
    assert_eq!(check(&release)?, "Recipes are already up to date (v4).\n");
    let Err(err) = checked(Ok(CheckAttempt::RetryLater), URL) else {
        anyhow::bail!("retry-later check succeeded");
    };
    assert!(err.to_string().contains("try again in a minute"), "{err}");
    Ok(())
}
