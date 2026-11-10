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
fn current_uid_owns_files_this_process_creates() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("f");
    std::fs::write(&path, b"x").unwrap();
    let meta = crate::linux::LinuxPlatform::from_env();
    let owner = crate::Platform::metadata(&meta, &path).unwrap().uid;
    assert_eq!(owner, Some(current_uid()));
}

#[test]
fn buffer_hint_names_the_sysctl_and_size() {
    let hint = udp_buffer_hint(8_388_608);
    assert!(hint.contains("net.core.rmem_max=8388608"), "{hint}");
}
