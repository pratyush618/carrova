//! Helpers shared by the transport's integration tests: loopback
//! pairing, in-memory chunk sources, manifests.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::{BTreeSet, HashMap};
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use carrova_crypto::pairing::{NAMEPLATES, Nameplate, PairingCode};
use carrova_manifest::{
    Artifact, Blake3Hash, ChunkHash, ChunkRef, ChunkSource, ChunkSourceError, ItemFlags, Manifest,
    ManifestItem, SourceInfo,
};
use carrova_platform::OsKind;
use carrova_transport::{
    AllowedChunks, ConnectOptions, Connection, ListenOptions, Listener, Target, Ticket, connect,
};
use serde::{Deserialize, Serialize};

pub const T: Duration = Duration::from_secs(10);
pub const LOOPBACK: Ipv4Addr = Ipv4Addr::LOCALHOST;

// ------------------------------------------------------------------ helpers

pub fn direct_listener() -> Listener {
    Listener::bind(ListenOptions {
        discovery: false,
        ..ListenOptions::default()
    })
    .unwrap()
}

pub fn discoverable_listener() -> Listener {
    Listener::bind(ListenOptions::default()).unwrap()
}

pub fn ticket(code: &PairingCode, listener: &Listener) -> Ticket {
    Ticket {
        code: code.clone(),
        addr: Some(SocketAddrV4::new(LOOPBACK, listener.port())),
    }
}

/// Nameplates handed out by [`unique_code`] in this test process.
static CLAIMED: NameplatePool = NameplatePool::new();

/// A pairing code whose nameplate no other [`unique_code`] caller in this
/// process holds. Discovery tests run in parallel on one host and find
/// senders by nameplate, so two tests on the same (randomly drawn)
/// nameplate would dial, guess at and burn each other's senders.
pub fn unique_code() -> PairingCode {
    CLAIMED.claim()
}

/// Nameplates already handed out; [`Self::claim`] draws fresh codes until
/// one has an unclaimed nameplate.
pub struct NameplatePool(Mutex<BTreeSet<Nameplate>>);

impl NameplatePool {
    pub const fn new() -> Self {
        Self(Mutex::new(BTreeSet::new()))
    }

    /// A random code on a nameplate not claimed from this pool before.
    pub fn claim(&self) -> PairingCode {
        let mut claimed = self.0.lock().unwrap();
        assert!(
            claimed.len() < NAMEPLATES.len(),
            "every nameplate is claimed"
        );
        loop {
            let code = PairingCode::generate().unwrap();
            if claimed.insert(code.nameplate()) {
                return code;
            }
        }
    }
}

/// A different code on the same nameplate.
pub fn other_words(code: &PairingCode) -> PairingCode {
    loop {
        let fresh = PairingCode::generate().unwrap().to_string();
        let words = fresh.split_once('-').unwrap().1;
        let other: PairingCode = format!("{}-{words}", code.nameplate()).parse().unwrap();
        if &other != code {
            return other;
        }
    }
}

/// Pair over loopback: `(sender, receiver, listener)`.
pub fn pair_on(listener: Listener) -> (Connection, Connection, Listener) {
    pair_with(listener, &ConnectOptions::default())
}

/// Pair over loopback, the receiver connecting with `opts`:
/// `(sender, receiver, listener)`.
pub fn pair_with(
    mut listener: Listener,
    opts: &ConnectOptions,
) -> (Connection, Connection, Listener) {
    let code = PairingCode::generate().unwrap();
    let t = ticket(&code, &listener);
    let sender = thread::spawn(move || {
        let conn = listener.accept_pairing(&code, T);
        (conn, listener)
    });
    let receiver = connect(Target::Pairing(&t), T, opts).unwrap();
    let (sender_conn, listener) = sender.join().unwrap();
    (sender_conn.unwrap(), receiver, listener)
}

pub fn pair() -> (Connection, Connection) {
    let (s, r, _) = pair_on(direct_listener());
    (s, r)
}

/// `n` distinct chunks of `size` bytes, tagged by `seed`.
pub fn make_chunks(seed: u8, n: usize, size: usize) -> Vec<(ChunkRef, Vec<u8>)> {
    let mut offset = 0;
    (0..n)
        .map(|i| {
            let mut bytes: Vec<u8> = (0..size)
                .map(|j| (j as u8).wrapping_mul(31) ^ seed)
                .collect();
            bytes[..8].copy_from_slice(&(i as u64).to_le_bytes());
            bytes[8] = seed;
            let chunk = ChunkRef {
                hash: ChunkHash::of(&bytes),
                offset,
                len: size as u32,
            };
            offset += size as u64;
            (chunk, bytes)
        })
        .collect()
}

pub fn manifest(chunks: &[ChunkRef]) -> Manifest {
    let size = chunks.iter().map(|c| u64::from(c.len)).sum();
    Manifest::new(
        0,
        SourceInfo {
            os: OsKind::Linux,
            carrova_version: "test".into(),
            device_name: None,
        },
        vec![ManifestItem {
            recipe_id: None,
            item_id: "blob".into(),
            label: "blob".into(),
            flags: ItemFlags::default(),
            entries: Vec::new(),
            handler: Some("test".into()),
            artifacts: vec![Artifact {
                name: "blob".into(),
                size,
                hash: Blake3Hash::of(b"unused"),
                chunks: chunks.to_vec(),
            }],
            source_paths: Vec::new(),
        }],
    )
}

/// In-memory chunk source counting reads per hash; optionally corrupts one
/// chunk or unwinds (simulated crash) after a number of reads.
#[derive(Default)]
pub struct MemSource {
    pub chunks: HashMap<ChunkHash, Vec<u8>>,
    pub reads: Arc<Mutex<HashMap<ChunkHash, usize>>>,
    pub corrupt: Option<ChunkHash>,
    pub crash_after: Option<usize>,
    pub total: usize,
}

impl MemSource {
    pub fn new(chunks: &[(ChunkRef, Vec<u8>)]) -> Self {
        Self {
            chunks: chunks.iter().map(|(c, b)| (c.hash, b.clone())).collect(),
            ..Self::default()
        }
    }
}

impl ChunkSource for MemSource {
    fn read_chunk(&mut self, chunk: &ChunkRef) -> Result<Vec<u8>, ChunkSourceError> {
        self.total += 1;
        if self.crash_after.is_some_and(|n| self.total > n) {
            // Unwind without the panic hook: the sender "process" dies.
            std::panic::resume_unwind(Box::new("simulated sender crash"));
        }
        *self.reads.lock().unwrap().entry(chunk.hash).or_default() += 1;
        let mut bytes = self
            .chunks
            .get(&chunk.hash)
            .cloned()
            .ok_or(ChunkSourceError::Missing(chunk.hash))?;
        if self.corrupt == Some(chunk.hash) {
            bytes[0] ^= 0xff;
        }
        Ok(bytes)
    }
}

/// Serve `source` for `allowed` on a thread.
pub fn serve(
    mut sender: Connection,
    allowed: AllowedChunks,
    mut source: MemSource,
) -> thread::JoinHandle<(
    carrova_transport::Result<carrova_transport::ServeEnd>,
    Connection,
)> {
    thread::spawn(move || {
        let end = sender.serve_chunks(&allowed, &mut source);
        (end, sender)
    })
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Ping {
    pub n: u32,
    #[serde(with = "serde_bytes")]
    pub blob: Vec<u8>,
}

pub fn control_round_trip(a: &mut Connection, b: &mut Connection) {
    let msg = Ping {
        n: 7,
        blob: vec![1, 2, 3],
    };
    a.send(&msg).unwrap();
    assert_eq!(b.recv::<Ping>(T).unwrap(), msg);
    b.send(&Ping { n: 8, blob: vec![] }).unwrap();
    assert_eq!(a.recv::<Ping>(T).unwrap().n, 8);
}

/// Broadcasts to `127.255.255.255` reach local sockets (Linux does; some
/// sandboxes and OSes don't). Discovery tests need it.
pub fn loopback_broadcast_works() -> bool {
    let probe = || -> std::io::Result<bool> {
        let rx = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        rx.set_read_timeout(Some(Duration::from_secs(1)))?;
        let port = rx.local_addr()?.port();
        let tx = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0))?;
        tx.set_broadcast(true)?;
        tx.send_to(b"probe", (Ipv4Addr::new(127, 255, 255, 255), port))?;
        let mut buf = [0u8; 8];
        Ok(rx.recv_from(&mut buf).is_ok())
    };
    let ok = probe().unwrap_or(false);
    if !ok {
        eprintln!("skipping: loopback broadcast unavailable in this environment");
    }
    ok
}
