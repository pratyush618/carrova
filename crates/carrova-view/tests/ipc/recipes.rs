//! `RecipesCheck` from a bundle check: the one mapping the CLI and the
//! desktop share.

use carrova_bundle::{BundleVersion, CheckAttempt, UpdateCheck};
use carrova_view::ipc::RecipesCheck;

#[test]
fn a_rolled_back_offer_is_reported_not_hidden_as_up_to_date() {
    let check = RecipesCheck::from(CheckAttempt::Done(UpdateCheck::Refused(BundleVersion(5))));
    assert_eq!(check, RecipesCheck::Refused { version: 5 });
}

#[test]
fn up_to_date_keeps_the_installed_version() {
    let check = RecipesCheck::from(CheckAttempt::Done(UpdateCheck::UpToDate(BundleVersion(4))));
    assert_eq!(check, RecipesCheck::UpToDate { version: 4 });
}

#[test]
fn an_unpublished_bundle_means_retry_later() {
    assert_eq!(
        RecipesCheck::from(CheckAttempt::RetryLater),
        RecipesCheck::RetryLater
    );
}

#[test]
fn refused_serialises_with_its_version() {
    let json = serde_json::to_value(RecipesCheck::Refused { version: 5 }).unwrap();
    assert_eq!(json, serde_json::json!({ "kind": "refused", "version": 5 }));
}
