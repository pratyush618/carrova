//! One supervisor per user: the first `attach_or_start` runs it and serves
//! `<state>/link.sock`; later ones become clients of it.

use std::io::{Read, Write};
use std::path::PathBuf;

use carrova_core::Engine;
use carrova_core::ecosystem::{
    ControlRequest, ControlResponse, MAX_CONTROL_MESSAGE, SupervisorHandle, attach_or_start,
};
use carrova_link::PolicyDoc;
use carrova_platform::Platform;

use crate::supervisor::device::{Device, pair};

/// A second engine (another process) of `device`'s user.
fn other_process(device: &Device) -> Engine {
    Engine::new(device.platform.clone(), Vec::new(), device.sink.clone())
}

fn socket(device: &Device) -> PathBuf {
    device.platform.state_dir().unwrap().join("link.sock")
}

#[test]
fn second_supervisor_becomes_client() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());

    let first = attach_or_start(low.engine.clone()).unwrap();
    let second = attach_or_start(other_process(&low)).unwrap();

    assert!(matches!(first, SupervisorHandle::Local(_)));
    assert!(matches!(second, SupervisorHandle::Remote(_)));
}

#[test]
fn client_status_matches_server() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    let server = attach_or_start(low.engine.clone()).unwrap();
    let client = attach_or_start(other_process(&low)).unwrap();

    let SupervisorHandle::Local(local) = &server else {
        panic!("first is not local")
    };
    let ControlResponse::Status(remote) = client.request(ControlRequest::Status).unwrap() else {
        panic!("no status")
    };

    assert_eq!(remote, local.supervisor().status());
    assert_eq!(remote.len(), 1);
    assert_eq!(remote[0].id, high.id);
}

#[test]
fn stale_socket_file_replaced() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    let path = socket(&low);
    // A socket left behind by a process that died: nobody listens on it.
    drop(low.platform.bind_local_socket(&path).unwrap());
    assert!(path.exists());

    let server = attach_or_start(low.engine.clone()).unwrap();
    let client = attach_or_start(other_process(&low)).unwrap();

    assert!(matches!(server, SupervisorHandle::Local(_)));
    assert!(matches!(
        client.request(ControlRequest::Status).unwrap(),
        ControlResponse::Status(_)
    ));
}

#[test]
fn oversize_request_rejected() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    let _server = attach_or_start(low.engine.clone()).unwrap();
    let mut raw = low.platform.connect_local_socket(&socket(&low)).unwrap();

    raw.write_all(&(MAX_CONTROL_MESSAGE + 1).to_be_bytes())
        .unwrap();
    let mut len = [0; 4];
    raw.read_exact(&mut len).unwrap();
    let mut body = vec![0; u32::from_be_bytes(len) as usize];
    raw.read_exact(&mut body).unwrap();
    let reply: ControlResponse = ciborium::from_reader(body.as_slice()).unwrap();

    assert!(matches!(reply, ControlResponse::Failed(_)), "{reply:?}");
    // The connection is closed, and the server still serves others.
    assert_eq!(raw.read(&mut len).unwrap(), 0);
    let client = attach_or_start(other_process(&low)).unwrap();
    assert!(matches!(
        client.request(ControlRequest::Status).unwrap(),
        ControlResponse::Status(_)
    ));
}

#[test]
fn request_debug_hides_the_path() {
    let (low, _high, _) = pair(&PolicyDoc::phone_defaults());
    let request = ControlRequest::SendFile(low.id, PathBuf::from("/home/me/secret-plan.pdf"));

    let shown = format!("{request:?}");

    assert!(!shown.contains("secret-plan"), "{shown}");
    assert!(shown.starts_with("SendFile"), "{shown}");
}
