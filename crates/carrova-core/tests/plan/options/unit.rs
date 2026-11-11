use super::*;

const WHY: Option<&str> = Some("holds private keys");

#[test]
fn without_a_list_every_item_is_picked_except_unconfirmed_sensitive_ones() {
    let options = PlanOptions {
        confirmed_sensitive: vec!["app/keys".into()],
        ..PlanOptions::default()
    };
    assert!(options.picks(&"app/config".into(), None, false));
    assert!(options.picks(&"app/keys".into(), WHY, false));
    assert!(!options.picks(&"app/token".into(), WHY, false));
}

#[test]
fn an_explicit_list_picks_exactly_its_items_whatever_their_sensitivity() {
    let options = PlanOptions {
        items: Some(vec!["app/token".into()]),
        ..PlanOptions::default()
    };
    assert!(options.picks(&"app/token".into(), WHY, false));
    assert!(!options.picks(&"app/config".into(), None, true));
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
fn accept_all_confirms_only_the_plans_safe_picks() {
    let options = PlanOptions {
        accept_safe_picks: true,
        ..PlanOptions::default()
    };
    assert!(options.confirms(&"raw:home/notes".into(), true));
    assert!(options.picks(&"raw:home/notes".into(), WHY, true));
    assert!(!options.confirms(&"raw:home/.ssh".into(), false));
    assert!(!options.picks(&"raw:home/.ssh".into(), WHY, false));
    assert!(!PlanOptions::default().confirms(&"raw:home/notes".into(), true));
}

#[test]
fn accept_all_defaults_off_and_older_stored_options_still_read() {
    let mut stored = serde_json::to_value(PlanOptions::default()).unwrap();
    let fields = stored.as_object_mut().unwrap();
    fields.remove("accept_safe_picks");
    fields.remove("declined");
    let read: PlanOptions = serde_json::from_value(stored).unwrap();
    assert_eq!(read, PlanOptions::default());
    assert!(!read.accept_safe_picks);
}

#[test]
fn cross_os_overrides_default_to_none_and_older_stored_options_still_read() {
    assert!(PlanOptions::default().allow_cross_os.is_empty());
    let mut stored = serde_json::to_value(PlanOptions::default()).unwrap();
    stored.as_object_mut().unwrap().remove("allow_cross_os");
    let read: PlanOptions = serde_json::from_value(stored).unwrap();
    assert_eq!(read, PlanOptions::default());
}
