#![allow(clippy::unwrap_used)]

use std::net::{Ipv4Addr, UdpSocket};
use std::time::Duration;

use carrova_core::lan::browse_nearby;
use carrova_core::presence::{Visibility, save_visibility};
use carrova_core::time::now_unix;
use carrova_platform::fake::FakePlatform;
use tempfile::TempDir;

use super::*;

/// A machine named `name` (`None`: unnamed) in its own temp dir.
fn machine(tmp: &TempDir, name: Option<&str>) -> Arc<FakePlatform> {
    let dir = tmp.path().join(name.unwrap_or("stranger"));
    let platform = FakePlatform::with_home(dir.join("home")).with_state_dir(dir.join("state"));
    Arc::new(match name {
        Some(name) => platform.with_device_name(name),
        None => platform,
    })
}

fn names_seen_by(platform: &FakePlatform) -> Vec<String> {
    browse_nearby(platform)
        .unwrap()
        .into_iter()
        .map(|d| d.name)
        .collect()
}

/// Whether this environment delivers loopback broadcasts (discovery).
fn loopback_broadcast_works() -> bool {
    let probe = || -> std::io::Result<bool> {
        let rx = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        rx.set_read_timeout(Some(Duration::from_secs(1)))?;
        let port = rx.local_addr()?.port();
        let tx = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        tx.set_broadcast(true)?;
        tx.send_to(b"probe", (Ipv4Addr::new(127, 255, 255, 255), port))?;
        Ok(rx.recv_from(&mut [0u8; 8]).is_ok())
    };
    probe().unwrap_or(false)
}

#[test]
fn start_refresh_and_stop_follow_the_saved_visibility() {
    if !loopback_broadcast_works() {
        return;
    }
    let tmp = TempDir::new().unwrap();
    let name = format!("desk-{}", now_unix() ^ u64::from(std::process::id()));
    let desk = machine(&tmp, Some(&name));
    let stranger = machine(&tmp, None);
    let presence = BackgroundPresence::new(desk.clone());

    presence.start();
    assert!(
        !names_seen_by(&stranger).contains(&name),
        "known devices only"
    );

    save_visibility(desk.as_ref(), Visibility::everyone_from(now_unix())).unwrap();
    presence.refresh();
    assert!(
        names_seen_by(&stranger).contains(&name),
        "shown once refreshed"
    );

    presence.stop();
    assert!(!names_seen_by(&stranger).contains(&name), "withdrawn");
}

#[test]
fn a_start_after_stop_does_nothing() {
    let tmp = TempDir::new().unwrap();
    let presence = BackgroundPresence::new(machine(&tmp, Some("desk")));
    presence.stop();
    presence.start();
    assert!(matches!(*presence.lock(), Running::Stopped));
}
