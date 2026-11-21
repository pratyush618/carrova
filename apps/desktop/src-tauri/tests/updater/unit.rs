use super::*;

#[test]
fn statuses_serialize_with_a_status_tag() {
    let available = UpdateStatus::Available(UpdateInfo {
        version: "0.2.0".into(),
        current_version: "0.1.0".into(),
        notes: Some("Fixes".into()),
        date: None,
    });
    let json = |status: &UpdateStatus| serde_json::to_value(status).unwrap();

    assert_eq!(
        json(&UpdateStatus::UpToDate),
        serde_json::json!({ "status": "up_to_date" })
    );
    assert_eq!(
        json(&available),
        serde_json::json!({
            "status": "available",
            "version": "0.2.0",
            "current_version": "0.1.0",
            "notes": "Fixes",
            "date": null
        })
    );
    assert_eq!(
        json(&UpdateStatus::Installed {
            version: "0.2.0".into()
        }),
        serde_json::json!({ "status": "installed", "version": "0.2.0" })
    );
}

#[test]
fn a_second_install_is_refused_and_keeps_the_first_ones_cancel() {
    let control = UpdateControl::default();
    let first = control.begin().unwrap();
    control.0.send_replace(Signal::Cancel);

    let second = control.begin().unwrap_err();
    assert_eq!(second.bridge().code, ErrorCode::Session);
    assert_eq!(*control.0.borrow(), Signal::Cancel);

    drop(first);
    assert!(control.begin().is_ok());
    assert_eq!(*control.0.borrow(), Signal::Go);
}

#[derive(Clone, Debug, PartialEq)]
struct FakeOffer(&'static str);

impl Offer for FakeOffer {
    fn version(&self) -> &str {
        self.0
    }
}

#[test]
fn installing_keeps_a_newer_offer_checked_meanwhile() {
    let pending = PendingUpdate::default();
    pending.replace(Some(FakeOffer("0.2.0")));
    // A check during the 0.2.0 install found 0.3.0.
    pending.replace(Some(FakeOffer("0.3.0")));

    pending.installed("0.2.0");
    assert_eq!(pending.offered("0.3.0"), Some(FakeOffer("0.3.0")));

    pending.installed("0.3.0");
    assert_eq!(pending.offered("0.3.0"), None);
}
