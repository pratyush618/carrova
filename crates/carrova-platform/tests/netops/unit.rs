use super::*;

#[test]
fn lan_interfaces_exclude_loopback_and_link_local() {
    for i in lan_ipv4_interfaces().unwrap() {
        assert!(!i.ip.is_loopback() && !i.ip.is_link_local(), "{i:?}");
    }
}

#[test]
fn broadcast_sockets_share_a_port() {
    let a = bind_broadcast_udp(0).unwrap();
    let port = a.local_addr().unwrap().port();
    let b = bind_broadcast_udp(port).unwrap();
    assert_eq!(b.local_addr().unwrap().port(), port);
    assert!(a.broadcast().unwrap() && b.broadcast().unwrap());
}

#[test]
fn buffered_socket_reports_granted_sizes() {
    let (socket, buffers) = bind_udp(0, 64 * 1024).unwrap();
    assert!(socket.local_addr().unwrap().is_ipv4());
    assert!(buffers.send > 0 && buffers.recv > 0);
}

#[test]
fn refused_buffer_request_steps_down_until_accepted() {
    let tried = std::cell::RefCell::new(Vec::new());
    let limit = 300 * 1024;
    request_buffer(1024 * 1024, |n| {
        tried.borrow_mut().push(n);
        if n > limit {
            Err(io::ErrorKind::OutOfMemory.into())
        } else {
            Ok(())
        }
    })
    .unwrap();
    assert_eq!(*tried.borrow(), [1024 * 1024, 512 * 1024, 256 * 1024]);
}

#[test]
fn buffer_request_gives_up_at_the_floor() {
    let err = request_buffer(256 * 1024, |_| Err(io::ErrorKind::OutOfMemory.into()));
    assert_eq!(err.unwrap_err().kind(), io::ErrorKind::OutOfMemory);
}

#[cfg(unix)]
#[test]
fn current_uid_owns_files_this_process_creates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    std::fs::write(&path, b"x").unwrap();
    let owner = crate::meta::lstat(&path).unwrap().uid;
    assert_eq!(owner, Some(current_uid()));
}

#[cfg(any(target_os = "linux", all(unix, feature = "fake")))]
#[test]
fn buffer_hint_names_the_sysctl_and_size() {
    let hint = udp_buffer_hint(8_388_608);
    assert!(hint.contains("net.core.rmem_max=8388608"), "{hint}");
}

#[test]
fn magic_packet_layout() {
    let mac = [1, 2, 3, 4, 5, 6];
    let p = magic_packet(mac);
    assert_eq!(p.len(), 102);
    assert_eq!(p[..6], [0xff; 6]);
    assert!(p[6..].chunks(6).all(|c| c == mac));
}
