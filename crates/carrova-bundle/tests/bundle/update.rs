use std::fs;

use carrova_bundle::{
    Asset, BlobKind, BundleError, BundleVersion, DirFetcher, InstallOutcome, RecipeEntry, Tier,
    UpdateCheck, UpdateOutcome, Updater, VerifyTime, open_bundle,
};

use crate::support::{CountingFetcher, Keys, NOW, bundle, edit_index, platform, store};

fn available(check: UpdateCheck) -> carrova_bundle::PendingUpdate {
    match check {
        UpdateCheck::Available(p) => p,
        other => panic!("expected an update, got {other:?}"),
    }
}

#[test]
fn built_bundle_round_trips_into_the_store() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");

    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let updater = Updater::new(DirFetcher::new(&out), keys.roots.clone());
    let UpdateOutcome::Installed {
        outcome,
        stats,
        skipped,
    } = updater.update(&store, NOW).unwrap()
    else {
        panic!("expected install");
    };
    assert_eq!(
        outcome,
        InstallOutcome::Installed {
            version: BundleVersion(1),
            previous: None
        }
    );
    assert_eq!((stats.fetched, stats.reused), (3, 0), "2 recipes + detect");
    assert!(skipped.is_empty());

    let catalog = store.recipes().unwrap();
    let tiers: Vec<(&str, Tier)> = catalog
        .recipes
        .iter()
        .map(|r| (r.recipe.id.as_str(), r.tier))
        .collect();
    assert_eq!(
        tiers,
        [("alpha", Tier::Verified), ("beta", Tier::Community)]
    );
    let detect = store.detect_index().unwrap();
    assert_eq!(detect.linux.keys().collect::<Vec<_>>(), ["alpha", "beta"]);
    assert!(
        store
            .root()
            .starts_with(tmp.path().join("home/.local/share/carrova/recipes"))
    );
}

#[test]
fn tampered_blob_is_rejected_and_nothing_installed() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");
    let (signed, _) =
        open_bundle(&DirFetcher::new(&out), &keys.roots, VerifyTime::Now(NOW)).unwrap();
    let entry = &signed.index().recipes[0];
    let path = out.join(entry.asset().file_name());
    let mut bytes = fs::read(&path).unwrap();
    bytes[0] ^= 1;
    fs::write(&path, bytes).unwrap();

    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let updater = Updater::new(DirFetcher::new(&out), keys.roots.clone());
    let err = updater.update(&store, NOW).unwrap_err();
    assert!(matches!(err, BundleError::BlobMismatch { .. }), "{err}");
    assert_eq!(store.installed().unwrap(), None);
}

#[test]
fn tampered_index_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");
    let path = out.join(Asset::INDEX_FILE);
    let text = fs::read_to_string(&path)
        .unwrap()
        .replace("\"version\": 1", "\"version\": 9");
    fs::write(&path, text).unwrap();

    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let updater = Updater::new(DirFetcher::new(&out), keys.roots.clone());
    let err = updater.check(&store, NOW).unwrap_err();
    assert!(matches!(err, BundleError::Signature(_)), "{err}");
}

#[test]
fn bundle_from_foreign_root_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let out = tmp.path().join("out");
    bundle(&Keys::new(), tmp.path(), &out, 1, "Alpha");
    let keys = Keys::new();
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let updater = Updater::new(DirFetcher::new(&out), keys.roots.clone());
    let err = updater.check(&store, NOW).unwrap_err();
    assert!(matches!(err, BundleError::Signature(_)), "{err}");
}

#[test]
fn older_version_is_refused_and_equal_is_up_to_date() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (v1, v2) = (tmp.path().join("v1"), tmp.path().join("v2"));
    bundle(&keys, tmp.path(), &v1, 1, "Alpha");
    bundle(&keys, tmp.path(), &v2, 2, "Alpha two");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    Updater::new(DirFetcher::new(&v2), keys.roots.clone())
        .update(&store, NOW)
        .unwrap();

    let old = Updater::new(DirFetcher::new(&v1), keys.roots.clone());
    let err = old.check(&store, NOW).unwrap_err();
    assert!(matches!(err, BundleError::Rollback { .. }), "{err}");
    let err = old.update(&store, NOW).unwrap_err();
    assert!(matches!(err, BundleError::Rollback { .. }), "{err}");

    let same = Updater::new(DirFetcher::new(&v2), keys.roots.clone());
    assert!(matches!(
        same.check(&store, NOW).unwrap(),
        UpdateCheck::UpToDate(BundleVersion(2))
    ));
}

#[test]
fn unsupported_schema_recipes_are_skipped_not_fatal() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");
    edit_index(&keys, &out, |index| {
        let mut future = index.recipes[0].clone();
        future.id = "gamma".into();
        future.schema = 99;
        index.recipes.push(future);
    });

    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let updater = Updater::new(CountingFetcher::new(&out), keys.roots.clone());
    let pending = available(updater.check(&store, NOW).unwrap());
    assert_eq!(pending.skipped().len(), 1);
    assert_eq!(pending.skipped()[0].id, "gamma");
    let ids: Vec<&str> = pending.recipes().iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["alpha", "beta"]);
    updater.fetch(&pending, &store).unwrap();
    store.install(pending.signed()).unwrap();
    let catalog = store.recipes().unwrap();
    assert_eq!(catalog.recipes.len(), 2);
    assert_eq!(
        catalog.skipped[0].reason,
        carrova_bundle::SkipCause::UnsupportedSchema { schema: 99 }
    );
}

#[test]
fn delta_fetch_requests_only_missing_blobs() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let (v1, v2) = (tmp.path().join("v1"), tmp.path().join("v2"));
    bundle(&keys, tmp.path(), &v1, 1, "Alpha");
    bundle(&keys, tmp.path(), &v2, 2, "Alpha, now better");
    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    Updater::new(DirFetcher::new(&v1), keys.roots.clone())
        .update(&store, NOW)
        .unwrap();

    let counting = CountingFetcher::new(&v2);
    let updater = Updater::new(&counting, keys.roots.clone());
    let pending = available(updater.check(&store, NOW).unwrap());
    let changed: Vec<&RecipeEntry> = pending
        .recipes()
        .into_iter()
        .filter(|e| e.id == "alpha")
        .collect();
    let stats = updater.fetch(&pending, &store).unwrap();
    assert_eq!(
        (stats.fetched, stats.reused),
        (1, 2),
        "detect + beta unchanged"
    );
    assert_eq!(
        counting.blobs(),
        [Asset::Blob(BlobKind::Recipe, changed[0].hash).file_name()]
    );
}

#[test]
fn malformed_hash_in_signed_index_is_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let keys = Keys::new();
    let out = tmp.path().join("out");
    bundle(&keys, tmp.path(), &out, 1, "Alpha");
    let (signed, _) =
        open_bundle(&DirFetcher::new(&out), &keys.roots, VerifyTime::Now(NOW)).unwrap();
    let hash = signed.index().recipes[0].hash.to_string();
    let path = out.join(Asset::INDEX_FILE);
    let evil = format!("../../../../.ssh/{}", &hash[16..]);
    let text = fs::read_to_string(&path).unwrap().replace(&hash, &evil);
    fs::write(&path, text).unwrap();
    keys.sign_raw(&out);

    let p = platform(&tmp.path().join("home"));
    let store = store(&p, &keys.roots);
    let err = Updater::new(DirFetcher::new(&out), keys.roots.clone())
        .check(&store, NOW)
        .unwrap_err();
    assert!(err.to_string().contains("malformed blob hash"), "{err}");
}
