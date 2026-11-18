//! Two linked devices in one process: each its own home, state dir,
//! recording platform, event sink and fake presence source.

use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::ecosystem::{PeerSource, PolicyStore, SeenPeer, Supervisor, SupervisorConfig};
use carrova_core::{Engine, Event};
use carrova_crypto::identity::{DeviceId, DeviceKey};
use carrova_crypto::pairing::KnownSecret;
use carrova_link::PolicyDoc;
use carrova_platform::OsKind;
use carrova_platform::fake::FakePlatform;
use carrova_transport::{ConnectOptions, Connection, Target, connect};
use tempfile::TempDir;

use super::recording::Recording;
use crate::lan_support::HookSink;

/// Every wait in these tests gives up after this long.
pub const T: Duration = Duration::from_secs(20);

/// A presence source the test controls; counts how often it was asked.
#[derive(Debug, Default)]
pub struct FakeSource {
    seen: Mutex<Vec<SeenPeer>>,
    asked: AtomicUsize,
}

impl FakeSource {
    pub fn set(&self, seen: Vec<SeenPeer>) {
        *self.seen.lock().unwrap_or_else(PoisonError::into_inner) = seen;
    }

    /// Wait until the supervisor asked `more` more times than now (it has
    /// run that many maintenance rounds).
    pub fn wait_rounds(&self, more: usize) {
        let target = self.asked.load(Ordering::SeqCst) + more;
        wait_until("maintenance rounds", || {
            self.asked.load(Ordering::SeqCst) >= target
        });
    }
}

impl PeerSource for FakeSource {
    fn seen(&self) -> Vec<SeenPeer> {
        self.asked.fetch_add(1, Ordering::SeqCst);
        self.seen
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

pub struct Device {
    _tmp: TempDir,
    pub home: PathBuf,
    pub platform: Arc<Recording>,
    pub sink: Arc<HookSink>,
    pub engine: Engine,
    pub id: DeviceId,
    pub source: Arc<FakeSource>,
}

impl Device {
    pub fn new(name: &str) -> Self {
        let tmp = TempDir::new().unwrap();
        let home = tmp.path().join("home");
        std::fs::create_dir_all(&home).unwrap();
        let fake = FakePlatform::with_home(&home)
            .with_state_dir(tmp.path().join("state/carrova"))
            .with_device_name(name);
        let platform = Arc::new(Recording::new(fake));
        let sink = Arc::new(HookSink::default());
        let engine = Engine::new(platform.clone(), Vec::new(), sink.clone());
        let id = KnownDevices::open(&*platform)
            .unwrap()
            .device_key(&*platform)
            .unwrap()
            .public();
        Self {
            _tmp: tmp,
            home,
            platform,
            sink,
            engine,
            id,
            source: Arc::new(FakeSource::default()),
        }
    }

    pub fn key(&self) -> Arc<DeviceKey> {
        let known = KnownDevices::open(&*self.platform).unwrap();
        Arc::new(known.device_key(&*self.platform).unwrap())
    }

    pub fn start(&self, config: SupervisorConfig) -> Supervisor {
        Supervisor::start_with(self.engine.clone(), self.source.clone(), config).unwrap()
    }

    pub fn set_policy(&self, peer: &DeviceId, doc: &PolicyDoc) {
        let store = PolicyStore::open(&*self.platform).unwrap();
        store.put(&*self.platform, peer, doc).unwrap();
    }

    /// This device's presence source reports `peer` at `sup`'s port.
    pub fn sees(&self, peer: &Device, sup: &Supervisor) {
        self.source.set(vec![SeenPeer {
            id: peer.id,
            addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, sup.port()),
        }]);
    }

    pub fn forget(&self, peer: &DeviceId) {
        let known = KnownDevices::open(&*self.platform).unwrap();
        assert!(known.forget(&*self.platform, peer).unwrap());
    }

    pub fn events(&self) -> Vec<Event> {
        self.sink.events()
    }

    pub fn count(&self, f: impl Fn(&Event) -> bool) -> usize {
        self.events().iter().filter(|e| f(e)).count()
    }

    /// Hold the first event matching `hold` in the sink until released,
    /// so its emitter waits there (see [`Device::clear_while_held`]).
    pub fn hold_first(&self, hold: impl Fn(&Event) -> bool + Send + 'static) -> Held {
        let (started, started_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel::<()>();
        let mut held = false;
        self.sink.set_hook(move |e| {
            if !held && hold(e) {
                held = true;
                started.send(()).unwrap();
                // Bounded only so a broken test fails instead of hanging.
                let _ = release_rx.recv_timeout(T);
            }
        });
        Held {
            started: started_rx,
            release,
        }
    }

    /// Once `held` is reached, run `clear` (which emits `cleared`) on
    /// another thread and release the held event only after `cleared`
    /// went out, or once `clear` is plainly waiting on the held emitter.
    /// An emitter outside the mirror lock thus emits after the clear; one
    /// under it holds the clear back until its event is out.
    pub fn clear_while_held(&self, held: &Held, cleared: &Event, clear: impl FnOnce() + Send) {
        held.started.recv_timeout(T).unwrap();
        thread::scope(|s| {
            let clearing = s.spawn(clear);
            let until = Instant::now() + Duration::from_secs(1);
            while !self.events().contains(cleared) && Instant::now() < until {
                thread::sleep(Duration::from_millis(10));
            }
            let _ = held.release.send(());
            clearing.join().unwrap();
        });
    }

    /// Whether an event matching `later` was emitted after `first` (which
    /// must have been).
    pub fn any_after(&self, first: &Event, later: impl Fn(&Event) -> bool) -> bool {
        let events = self.events();
        let at = events.iter().position(|e| e == first).unwrap();
        events[at..].iter().any(later)
    }

    pub fn wait(&self, what: &str, f: impl Fn(&Event) -> bool) -> Event {
        let mut found = None;
        wait_until(what, || {
            found = self.events().into_iter().find(&f);
            found.is_some()
        });
        found.unwrap()
    }

    /// Dial `peer`'s supervisor as this device, outside any supervisor.
    pub fn dial(&self, peer: &Device, sup: &Supervisor, secret: &KnownSecret) -> Connection {
        connect(
            Target::Link {
                peer: &peer.id,
                secret,
                addr: SocketAddrV4::new(Ipv4Addr::LOCALHOST, sup.port()),
            },
            T,
            &ConnectOptions {
                platform: self.platform.clone(),
                device: Some(self.key()),
                cancel: None,
            },
        )
        .unwrap()
    }
}

/// Poll `done` until it holds; panics after [`T`].
pub fn wait_until(what: &str, mut done: impl FnMut() -> bool) {
    let until = Instant::now() + T;
    while !done() {
        assert!(Instant::now() < until, "timed out waiting for {what}");
        thread::sleep(Duration::from_millis(20));
    }
}

/// Two devices that know each other (one shared secret), each on `doc`
/// for the other: `(low, high, secret)`, the low key dials (ADR 0002 §4).
pub fn pair(doc: &PolicyDoc) -> (Device, Device, KnownSecret) {
    let low = Device::new("Low");
    // Each must announce the name its peer knows it by (the policy lane
    // carries it), so make devices until "High" has the higher key.
    let high = std::iter::repeat_with(|| Device::new("High"))
        .find(|high| high.id.as_bytes() > low.id.as_bytes())
        .unwrap();
    let secret = KnownSecret::from_bytes([7; 32]);
    low.know(&high, "High", &secret, doc);
    high.know(&low, "Low", &secret, doc);
    (low, high, secret)
}

impl Device {
    /// Remember `other` as `name`, linked with `secret`, on `doc` for it.
    pub fn know(&self, other: &Device, name: &str, secret: &KnownSecret, doc: &PolicyDoc) {
        let known = KnownDevices::open(&*self.platform).unwrap();
        let record = DeviceRecord {
            public: other.id,
            name: name.to_owned(),
            os: OsKind::Linux,
            first_seen: 1,
            last_move: None,
            moves: 0,
            mac_hint: None,
            renamed: false,
        };
        known.remember(&*self.platform, record, secret).unwrap();
        self.set_policy(&other.id, doc);
    }
}

/// Fast rounds and backoff, so the tests run in seconds.
pub fn fast() -> SupervisorConfig {
    SupervisorConfig {
        tick: Duration::from_millis(50),
        backoff_min: Duration::from_millis(50),
        backoff_max: Duration::from_millis(400),
        ..SupervisorConfig::default()
    }
}

/// Both supervisors started, each seeing the other: `(low's, high's)`.
pub fn start_both(low: &Device, high: &Device) -> (Supervisor, Supervisor) {
    let (sl, sh) = (low.start(fast()), high.start(fast()));
    low.sees(high, &sh);
    high.sees(low, &sl);
    (sl, sh)
}

/// An event held in a device's sink by [`Device::hold_first`].
pub struct Held {
    started: mpsc::Receiver<()>,
    release: mpsc::Sender<()>,
}
