//! `carrova devices` and `carrova visibility`, end to end through the
//! binary (HOME and Carrova's state dir in a temp dir). Linux only
//! (`XDG_STATE_HOME` places the state dir; returns early elsewhere).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use carrova_core::devices::KnownDevices;
use carrova_core::ecosystem::offer_pairing;
use carrova_core::presence::EVERYONE_FOR;
use carrova_core::time::now_unix;
use carrova_core::{Cancel, Engine, NullSink, Ticket};
use carrova_crypto::identity::Fingerprint;
use carrova_platform::fake::FakePlatform;
use serde_json::Value;
use tempfile::TempDir;

use support::User;

/// Names of the devices `carrova devices list` shows.
fn names(user: &User) -> Vec<String> {
    user.json(&["devices", "list"])
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["name"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn devices_list_shows_fingerprints() {
    let Some(user) = User::new() else { return };
    assert_eq!(user.ok(&["devices", "list"]), "No known devices.\n");
    let desk = user.remember(1, "Desk");

    let listed = user.ok(&["devices"]);

    let fingerprint = Fingerprint::of(&desk).to_string();
    assert!(listed.contains(&fingerprint), "{listed}");
    assert!(listed.contains("Desk"), "{listed}");
    assert!(
        !listed.contains(&desk.to_hex()),
        "never the full key: {listed}"
    );
}

#[test]
fn devices_rename_and_forget_by_fingerprint() {
    let Some(user) = User::new() else { return };
    let desk = user.remember(1, "Desk");
    user.remember(2, "Laptop");
    let fingerprint = Fingerprint::of(&desk).to_string();

    user.ok(&["devices", "rename", &fingerprint, "Office PC"]);
    assert_eq!(names(&user), ["Office PC", "Laptop"]);

    user.ok(&["devices", "forget", &fingerprint]);
    assert_eq!(names(&user), ["Laptop"]);
}

#[test]
fn waking_a_device_without_a_hint_says_so() {
    let Some(user) = User::new() else { return };
    let desk = user.remember(1, "Desk");

    let woke = user.carrova(&["devices", "wake", &Fingerprint::of(&desk).to_string()]);

    assert!(!woke.status.success());
    let stderr = String::from_utf8_lossy(&woke.stderr);
    assert!(stderr.contains("no wake hint saved"), "{stderr}");
}

#[test]
fn forgetting_every_device_needs_confirmation() {
    let Some(user) = User::new() else { return };
    user.remember(1, "Desk");

    let refused = user.carrova(&["devices", "forget", "--all"]);
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("--yes"));
    assert_eq!(names(&user), ["Desk"]);

    user.ok(&["devices", "forget", "--all", "--yes"]);
    assert!(names(&user).is_empty());
}

#[test]
fn visibility_everyone_sets_deadline() {
    let Some(user) = User::new() else { return };
    assert_eq!(user.json(&["visibility"])["choice"], "known");

    let before = now_unix();
    let set = user.json(&["visibility", "everyone"]);

    assert_eq!(set["choice"], "everyone");
    let until = set["until"].as_u64().unwrap();
    assert!(
        (before + EVERYONE_FOR.as_secs()..=now_unix() + EVERYONE_FOR.as_secs()).contains(&until),
        "{until}"
    );
    assert_eq!(user.json(&["visibility"]), set, "kept for the next command");
    let shown = user.ok(&["visibility"]);
    assert!(shown.contains("everyone until"), "{shown}");
    assert!(
        shown.contains("within seconds while the Carrova app runs"),
        "{shown}"
    );

    user.ok(&["visibility", "no-one"]);
    assert_eq!(user.json(&["visibility"])["choice"], "no_one");
}

/// The `local` setting of each feature row in `rows`.
fn local_states(rows: &Value) -> Vec<(String, String)> {
    rows.as_array()
        .unwrap()
        .iter()
        .map(|r| {
            let field = |k: &str| r[k].as_str().unwrap().to_owned();
            (field("feature"), field("local"))
        })
        .collect()
}

#[test]
fn devices_features_set_and_show() {
    let Some(user) = User::new() else { return };
    let desk = Fingerprint::of(&user.remember(1, "Desk")).to_string();

    let set = user.json(&["devices", "features", &desk, "clipboard=on", "files=ask"]);

    let expected = [
        ("clipboard", "on"),
        ("ring", "off"),
        ("files", "ask"),
        ("handoff", "off"),
        ("notifications", "off"),
        ("messages", "off"),
        ("calls", "off"),
    ]
    .map(|(f, s)| (f.to_owned(), s.to_owned()));
    assert_eq!(local_states(&set), expected);
    let shown = user.json(&["devices", "features", &desk]);
    assert_eq!(local_states(&shown), expected, "kept for the next command");
    let text = user.ok(&["devices", "features", &desk]);
    assert!(text.contains("clipboard"), "{text}");
}

#[test]
fn devices_features_sets_notifications() {
    let Some(user) = User::new() else { return };
    let desk = Fingerprint::of(&user.remember(1, "Desk")).to_string();

    let set = user.json(&["devices", "features", &desk, "notifications=on"]);

    let states = local_states(&set);
    for (feature, state) in [
        ("notifications", "on"),
        ("messages", "off"),
        ("calls", "off"),
    ] {
        assert!(
            states.contains(&(feature.to_owned(), state.to_owned())),
            "{feature}={state} in {states:?}"
        );
    }
}

#[test]
fn features_bad_pair_exit_2() {
    let Some(user) = User::new() else { return };
    let desk = Fingerprint::of(&user.remember(1, "Desk")).to_string();

    for bad in ["clipboard", "clipboard=maybe", "teleport=on"] {
        let out = user.carrova(&["devices", "features", &desk, bad]);

        assert_eq!(out.status.code(), Some(2), "{bad}");
        let stderr = String::from_utf8_lossy(&out.stderr);
        assert!(
            stderr.contains("expected <feature>=<on|off|ask>"),
            "{bad}: {stderr}"
        );
    }
}

/// Wait up to `limit` for `done`.
fn eventually(limit: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    while start.elapsed() < limit {
        if done() {
            return true;
        }
        thread::sleep(Duration::from_millis(100));
    }
    false
}

#[test]
fn devices_link_with_ticket_remembers() {
    let Some(user) = User::new() else { return };
    let mut serve = user.spawn(&["link", "serve"]);
    let socket = user.state_dir().join("link.sock");
    assert!(
        eventually(Duration::from_secs(10), || socket.exists()),
        "`carrova link serve` did not start (running: {})",
        serve.running()
    );
    // The other computer, in this process, offers a code on loopback.
    let host = TempDir::new().unwrap();
    let platform = Arc::new(
        FakePlatform::with_home(host.path().join("home"))
            .with_state_dir(host.path().join("state/carrova"))
            .with_device_name("Host"),
    );
    let engine = Engine::new(platform.clone(), Vec::new(), Arc::new(NullSink));
    let (tx, rx) = mpsc::channel();
    let offer = thread::spawn(move || {
        offer_pairing(
            &engine,
            &Cancel::default(),
            |invite| tx.send(invite.ticket).unwrap(),
            |_| true,
        )
    });
    let offered = rx.recv_timeout(Duration::from_secs(10)).unwrap();
    let port = offered.addr.unwrap().port();
    let ticket = Ticket {
        code: offered.code,
        addr: Some(std::net::SocketAddrV4::new(
            std::net::Ipv4Addr::LOCALHOST,
            port,
        )),
    };

    let linked = user.json(&["devices", "link", &ticket.to_string()]);

    assert_eq!(linked["name"], "Host");
    let on_host = offer.join().unwrap().unwrap();
    let remembered = KnownDevices::open(&*platform)
        .unwrap()
        .list(&*platform)
        .unwrap();
    assert_eq!(remembered, [on_host]);
    assert_eq!(names(&user), ["Host"]);
    // The running supervisor picks the new device up on its next round.
    let status_names = || {
        user.json(&["link", "status"])
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["name"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };
    assert!(
        eventually(Duration::from_secs(10), || status_names() == ["Host"]),
        "{:?}",
        status_names()
    );
    assert!(serve.running(), "status attached to the served supervisor");
}
