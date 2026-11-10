use super::*;

const WHY: Option<&str> = Some("holds private keys");

#[test]
fn without_a_list_every_item_is_picked_except_unconfirmed_sensitive_ones() {
    let options = PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    };
    assert!(options.picks(&"app/config".into(), None));
    assert!(options.picks(&"app/keys".into(), WHY));
    assert!(!options.picks(&"app/token".into(), WHY));
    assert!(options.awaits_confirmation(&"app/token".into(), WHY));
    assert!(!options.awaits_confirmation(&"app/keys".into(), WHY));
}

#[test]
fn an_explicit_list_picks_exactly_its_items_whatever_their_sensitivity() {
    let options = PlanOptions {
        items: Some(vec!["app/token".into()]),
        ..PlanOptions::default()
    };
    assert!(options.picks(&"app/token".into(), WHY));
    assert!(!options.picks(&"app/config".into(), None));
}

#[test]
fn confirmation_is_recorded_per_item() {
    let options = PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    };
    assert!(options.is_confirmed(&"app/keys".into()));
    assert!(!options.is_confirmed(&"app/token".into()));
}

#[test]
fn cross_os_overrides_default_to_none_and_older_stored_options_still_read() {
    assert!(PlanOptions::default().allow_cross_os.is_empty());
    let mut stored = serde_json::to_value(PlanOptions::default()).unwrap();
    stored.as_object_mut().unwrap().remove("allow_cross_os");
    let read: PlanOptions = serde_json::from_value(stored).unwrap();
    assert_eq!(read, PlanOptions::default());
}
