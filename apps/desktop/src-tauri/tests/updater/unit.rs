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
