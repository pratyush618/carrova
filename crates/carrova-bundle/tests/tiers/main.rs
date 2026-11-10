//! `TierPolicy`: listed ids are verified, unknown ids are rejected.

use carrova_bundle::{Tier, TierPolicy};

#[test]
fn listed_ids_are_verified_and_others_community() {
    let p = TierPolicy::parse("verified: [git, ssh]\n").unwrap();
    assert_eq!(p.tier_of("git"), Tier::Verified);
    assert_eq!(p.tier_of("vim"), Tier::Community);
}

#[test]
fn unknown_listed_id_is_an_error() {
    let p = TierPolicy::parse("verified: [git, gti]\n").unwrap();
    let err = p.check_known(["git"]).unwrap_err();
    assert!(err.to_string().contains("gti"), "{err}");
    assert!(TierPolicy::parse("verfied: [git]\n").is_err());
}
