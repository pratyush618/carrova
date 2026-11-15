//! Lane streams on a connection (public API): both sides open them,
//! frames and raw bytes share one, move-session chunk streams never
//! surface as lanes, and a link's keep-alive holds an idle connection.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use carrova_crypto::identity::{DeviceKey, DevicePublic};
use carrova_crypto::pairing::KnownSecret;
use carrova_platform::{
    FileMeta, OsKind, Platform, PlatformError, ProcessInfo, SemanticRoot, SnapshotCapability,
    SocketBuffers,
};
use carrova_transport::{
    ConnectOptions, Connection, KnownPeers, ListenOptions, Listener, MAX_FRAME_LEN, Stream, Target,
    TransportError, connect,
};

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod support;
use support::*;

const SHORT: Duration = Duration::from_millis(300);

/// Open a lane on `opener` while `acceptor` accepts it: (opened, accepted).
fn lane(opener: &Connection, acceptor: &Connection) -> (Stream, Stream) {
    thread::scope(|s| {
        let accepted = s.spawn(|| acceptor.accept_stream(T).unwrap());
        let opened = opener.open_stream().unwrap();
        (opened, accepted.join().unwrap())
    })
}

#[test]
fn both_sides_open_streams() {
    let (sender, receiver) = pair();

    let (mut a, mut b) = lane(&sender, &receiver);
    a.send(&"from sender".to_owned()).unwrap();
    assert_eq!(b.recv::<String>(T).unwrap(), "from sender");
    b.send(&7u32).unwrap();
    assert_eq!(a.recv::<u32>(T).unwrap(), 7);

    let (mut c, mut d) = lane(&receiver, &sender);
    c.send(&"from receiver".to_owned()).unwrap();
    assert_eq!(d.recv::<String>(T).unwrap(), "from receiver");
}

#[test]
fn frames_and_raw_bytes_on_one_stream() {
    let (sender, receiver) = pair();
    let (mut a, mut b) = lane(&sender, &receiver);
    let raw: Vec<u8> = (0..=255).cycle().take(300_000).collect();

    a.send(&"header".to_owned()).unwrap();
    a.write_all(&raw).unwrap();
    a.send(&"trailer".to_owned()).unwrap();
    a.finish().unwrap();

    assert_eq!(b.recv::<String>(T).unwrap(), "header");
    let mut got = vec![0u8; raw.len()];
    b.read_exact(&mut got, T).unwrap();
    assert_eq!(got, raw);
    assert_eq!(b.recv::<String>(T).unwrap(), "trailer");
    assert!(matches!(
        b.recv::<String>(T),
        Err(TransportError::StreamFinished)
    ));
}

#[test]
fn read_exact_times_out_without_losing_bytes() {
    let (sender, receiver) = pair();
    let (mut a, mut b) = lane(&sender, &receiver);

    a.write_all(b"half").unwrap();
    let mut buf = [0u8; 8];
    assert!(matches!(
        b.read_exact(&mut buf, SHORT),
        Err(TransportError::Timeout)
    ));
    a.write_all(b"-done").unwrap();
    b.read_exact(&mut buf, T).unwrap();
    assert_eq!(&buf, b"half-don");
}

#[test]
fn frame_cap_enforced() {
    let (sender, receiver) = pair();
    let (mut a, mut b) = lane(&sender, &receiver);

    let huge = "x".repeat(MAX_FRAME_LEN + 1);
    assert!(matches!(
        a.send(&huge),
        Err(TransportError::FrameTooLarge { .. })
    ));

    // A peer announcing an oversized frame is refused before its body.
    let len = u32::try_from(MAX_FRAME_LEN + 1).unwrap();
    a.write_all(&len.to_be_bytes()).unwrap();
    assert!(matches!(
        b.recv::<String>(T),
        Err(TransportError::FrameTooLarge { len }) if len == MAX_FRAME_LEN + 1
    ));
}

#[test]
fn accept_stream_times_out() {
    let (sender, receiver) = pair();
    assert!(matches!(
        receiver.accept_stream(SHORT),
        Err(TransportError::Timeout)
    ));
    // The connection is still usable afterwards.
    let (mut a, mut b) = lane(&sender, &receiver);
    a.send(&1u8).unwrap();
    assert_eq!(b.recv::<u8>(T).unwrap(), 1);
}

#[test]
fn chunk_streams_not_seen_as_lanes() {
    let (sender, mut receiver) = pair();
    let chunks = make_chunks(1, 4, 1024);
    let refs: Vec<_> = chunks.iter().map(|(c, _)| *c).collect();

    // The receiver opens chunk streams (and the fetch-done stream); the
    // sender, not serving, only looks for lanes.
    let fetch = thread::spawn(move || {
        let _ = receiver.fetch_chunks(&refs, 2, &mut |_, _| Ok(()));
        receiver
    });
    assert!(matches!(
        sender.accept_stream(Duration::from_secs(1)),
        Err(TransportError::Timeout)
    ));
    drop(sender);
    fetch.join().unwrap();
}

#[test]
fn connection_is_shareable_across_threads() {
    fn assert_sync<T: Sync + Send>() {}
    assert_sync::<Connection>();
    assert_sync::<Stream>();
}

// ------------------------------------------------------------------ keepalive

/// The real platform with a short link keep-alive.
#[derive(Debug)]
struct FastKeepalive(Box<dyn Platform>);

impl FastKeepalive {
    const INTERVAL: Duration = Duration::from_millis(200);

    fn shared() -> Arc<dyn Platform> {
        Arc::new(Self(carrova_platform::current()))
    }
}

impl Platform for FastKeepalive {
    fn os(&self) -> OsKind {
        self.0.os()
    }
    fn resolve_root(&self, root: SemanticRoot) -> Result<PathBuf, PlatformError> {
        self.0.resolve_root(root)
    }
    fn find_binary(&self, name: &str) -> Result<Option<PathBuf>, PlatformError> {
        self.0.find_binary(name)
    }
    fn metadata(&self, path: &Path) -> Result<FileMeta, PlatformError> {
        self.0.metadata(path)
    }
    fn running_processes(&self) -> Result<Vec<ProcessInfo>, PlatformError> {
        self.0.running_processes()
    }
    fn free_space(&self, path: &Path) -> Result<u64, PlatformError> {
        self.0.free_space(path)
    }
    fn snapshot_capability(&self, path: &Path) -> Result<SnapshotCapability, PlatformError> {
        self.0.snapshot_capability(path)
    }
    fn bind_udp(&self, port: u16, buffer: usize) -> io::Result<(UdpSocket, SocketBuffers)> {
        self.0.bind_udp(port, buffer)
    }
    fn link_keepalive(&self) -> Duration {
        Self::INTERVAL
    }
}

struct Known(DevicePublic, KnownSecret);

impl KnownPeers for Known {
    fn candidates(&self) -> Vec<(DevicePublic, KnownSecret)> {
        vec![(self.0, self.1.clone())]
    }
}

/// Link two fresh devices over loopback on `platform`: (listener side,
/// dialler side).
fn link(platform: &Arc<dyn Platform>) -> (Connection, Connection) {
    let (a, b) = (
        Arc::new(DeviceKey::generate().unwrap()),
        Arc::new(DeviceKey::generate().unwrap()),
    );
    let secret = KnownSecret::from_bytes([9; 32]);
    let mut listener = Listener::bind(ListenOptions {
        discovery: false,
        device: Some(Arc::clone(&a)),
        platform: Arc::clone(platform),
        ..ListenOptions::default()
    })
    .unwrap();
    let addr = SocketAddrV4::new(Ipv4Addr::LOCALHOST, listener.port());
    let known = Arc::new(Known(b.public(), secret.clone()));
    let accepted = thread::spawn(move || listener.accept_link(known, T).unwrap());
    let dialled = connect(
        Target::Link {
            peer: &a.public(),
            secret: &secret,
            addr,
        },
        T,
        &ConnectOptions {
            platform: Arc::clone(platform),
            device: Some(b),
            cancel: None,
        },
    )
    .unwrap();
    (accepted.join().unwrap(), dialled)
}

#[test]
fn keepalive_keeps_idle_link() {
    let (listening, dialled) = link(&FastKeepalive::shared());

    // Idle for 10× the keep-alive, over 3× the idle timeout.
    thread::sleep(FastKeepalive::INTERVAL * 10);

    let (mut a, mut b) = lane(&dialled, &listening);
    a.send(&"still here".to_owned()).unwrap();
    assert_eq!(b.recv::<String>(T).unwrap(), "still here");
}
