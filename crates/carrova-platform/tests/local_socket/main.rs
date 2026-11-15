//! Local (same-user) sockets: mode `0600`, peer-uid check against
//! `current_uid()`.
#![cfg(all(target_os = "linux", feature = "fake"))]
#![allow(clippy::unwrap_used)]

use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::thread;

use carrova_platform::Platform;
use carrova_platform::fake::{FakePlatform, Hooks};

#[test]
fn local_socket_roundtrip() {
    let tmp = tempfile::tempdir().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    let path = tmp.path().join("ctl.sock");
    let listener = p.bind_local_socket(&path).unwrap();
    let server = thread::spawn(move || {
        let mut s = listener.accept().unwrap();
        let mut buf = [0u8; 4];
        s.read_exact(&mut buf).unwrap();
        s.write_all(&buf).unwrap();
    });
    let mut c = p.connect_local_socket(&path).unwrap();
    c.write_all(b"ping").unwrap();
    let mut buf = [0u8; 4];
    c.read_exact(&mut buf).unwrap();
    assert_eq!(&buf, b"ping");
    server.join().unwrap();
}

#[test]
fn local_socket_mode_0600() {
    let tmp = tempfile::tempdir().unwrap();
    let p = FakePlatform::with_home(tmp.path());
    let path = tmp.path().join("ctl.sock");
    let _listener = p.bind_local_socket(&path).unwrap();
    let mode = std::fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o777, 0o600);
}

#[test]
fn local_socket_rejects_other_uid() {
    let tmp = tempfile::tempdir().unwrap();
    let real = FakePlatform::with_home(tmp.path());
    let me = real.current_uid().unwrap();
    // The listener believes it belongs to another user, so our own
    // connection counts as a foreign peer.
    let other = real.with_hooks(Hooks {
        uid: Some(me + 1),
        ..Hooks::default()
    });
    let path = tmp.path().join("ctl.sock");
    let listener = other.bind_local_socket(&path).unwrap();
    let _client = other.connect_local_socket(&path).unwrap();
    assert!(listener.accept().is_err());
}
