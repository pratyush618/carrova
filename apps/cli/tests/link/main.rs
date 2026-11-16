//! `carrova link`, `clip`, `file` and `handoff`, end to end through the
//! binary (HOME and Carrova's state dir in a temp dir). Linux only
//! (returns early elsewhere).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod support;

use carrova_crypto::identity::Fingerprint;
use serde_json::json;

use support::User;

#[test]
fn link_status_json_shape() {
    let Some(user) = User::new() else { return };
    let desk = user.remember(1, "Desk");

    let status = user.json(&["link", "status"]);

    let rows = [
        "clipboard",
        "ring",
        "files",
        "handoff",
        "notifications",
        "messages",
        "calls",
    ]
    .map(|feature| {
        let group = if matches!(feature, "files" | "handoff") {
            "ask"
        } else {
            "quiet"
        };
        json!({
            "feature": feature,
            "group": group,
            "local": "off",
            "remote": null,
            "effective": "off",
        })
    });
    assert_eq!(
        status,
        json!([{
            "id": desk.to_hex(),
            "name": "Desk",
            "os": "linux",
            "fingerprint": Fingerprint::of(&desk).to_string(),
            "state": "away",
            "features": rows,
        }])
    );
}

#[test]
fn clip_send_requires_linked_device() {
    let Some(user) = User::new() else { return };
    user.remember(1, "Desk");
    let stranger = Fingerprint::of(&user.remember(2, "Gone")).to_string();
    user.ok(&["devices", "forget", &stranger]);

    let unknown = user.carrova(&["clip", "send", "--to", &stranger]);
    assert!(!unknown.status.success());
    let stderr = String::from_utf8_lossy(&unknown.stderr);
    assert!(stderr.contains("no known device matches"), "{stderr}");

    let desk = Fingerprint::of(&user.remember(1, "Desk")).to_string();
    let away = user.carrova(&["clip", "send", "--to", &desk]);
    assert!(!away.status.success());
    let stderr = String::from_utf8_lossy(&away.stderr);
    assert!(stderr.contains("not connected"), "{stderr}");
}
