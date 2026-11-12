//! Wake-on-LAN hints: saved only when the setting is on, used by `wake`,
//! never logged.

use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock, PoisonError};

use carrova_core::devices::{DeviceRecord, KnownDevices};
use carrova_core::{CoreError, Engine};
use carrova_crypto::identity::DeviceKey;
use carrova_crypto::pairing::KnownSecret;
use carrova_platform::OsKind;
use carrova_platform::fake::{FakePlatform, Hooks};

use super::lan_support::Side;
use super::remember::{known, moved};

const MAC: [u8; 6] = [0x02, 0xca, 0xfe, 0xf0, 0x0d, 0x42];

/// `side` on a LAN where every neighbour has hardware address `MAC`.
fn on_lan(mut side: Side) -> Side {
    side.platform = Arc::new((*side.platform).clone().with_hooks(Hooks {
        neighbour_mac: Some(MAC),
        ..Hooks::default()
    }));
    side.engine = Engine::new(
        side.platform.clone(),
        side.engine.recipes().to_vec(),
        side.sink.clone(),
    );
    side
}

fn hint_of(side: &Side) -> Option<[u8; 6]> {
    let [record] = known(side).try_into().unwrap();
    record.mac_hint
}

#[test]
fn hint_saved_only_when_setting_on() {
    let receiver = Side::new();
    let off = on_lan(Side::sender());
    moved(&off, &receiver);
    assert_eq!(hint_of(&off), None);

    let on = on_lan(Side::sender());
    let on = Side {
        engine: on.engine.clone().with_wol_hints(true),
        ..on
    };
    moved(&on, &receiver);
    assert_eq!(hint_of(&on), Some(MAC));
}

#[test]
fn turning_hints_off_clears_saved_hint_on_next_move() {
    let receiver = Side::new();
    let on = on_lan(Side::sender());
    let on = Side {
        engine: on.engine.clone().with_wol_hints(true),
        ..on
    };
    moved(&on, &receiver);
    assert_eq!(hint_of(&on), Some(MAC));

    let off = Side {
        engine: on.engine.clone().with_wol_hints(false),
        ..on
    };
    moved(&off, &receiver);
    assert_eq!(hint_of(&off), None);
}

#[test]
fn empty_lookup_keeps_hint_while_on() {
    let receiver = Side::new();
    let on = on_lan(Side::sender());
    let on = Side {
        engine: on.engine.clone().with_wol_hints(true),
        ..on
    };
    moved(&on, &receiver);
    // The same machine, now with an empty neighbour table.
    let platform = Arc::new(
        FakePlatform::with_home(&on.home)
            .with_state_dir(on.tmp.path().join("state/carrova"))
            .binary("app", "/usr/bin/app"),
    );
    let blind = Side {
        engine: Engine::new(
            platform.clone(),
            on.engine.recipes().to_vec(),
            on.sink.clone(),
        )
        .with_wol_hints(true),
        platform,
        ..on
    };
    moved(&blind, &receiver);
    assert_eq!(hint_of(&blind), Some(MAC));
}

struct Store {
    _tmp: tempfile::TempDir,
    platform: FakePlatform,
    devices: KnownDevices,
}

impl Store {
    fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let platform = FakePlatform::with_home(tmp.path())
            .with_state_dir(tmp.path().join("state"))
            .with_hooks(Hooks::default());
        let devices = KnownDevices::open(&platform).unwrap();
        Self {
            _tmp: tmp,
            platform,
            devices,
        }
    }

    /// Remember a new device with `hint`; returns its id.
    fn remember(&self, hint: Option<[u8; 6]>) -> carrova_crypto::identity::DeviceId {
        let public = DeviceKey::generate().unwrap().public();
        let record = DeviceRecord {
            public,
            name: "Laptop".into(),
            os: OsKind::Linux,
            first_seen: 1,
            last_move: None,
            moves: 0,
            mac_hint: hint,
        };
        let secret = KnownSecret::from_bytes([7; 32]);
        self.devices
            .remember(&self.platform, record, &secret)
            .unwrap();
        public
    }
}

#[test]
fn wake_without_hint_is_no_wake_hint() {
    let s = Store::new();
    let id = s.remember(None);
    let err = s.devices.wake(&s.platform, &id).unwrap_err();
    assert!(matches!(err, CoreError::NoWakeHint), "{err}");
    assert!(s.platform.magic_packets().is_empty());

    let stranger = DeviceKey::generate().unwrap().public();
    let err = s.devices.wake(&s.platform, &stranger).unwrap_err();
    assert!(matches!(err, CoreError::UnknownDevice), "{err}");
}

#[test]
fn clear_wake_hints_removes_all() {
    let s = Store::new();
    let (a, b) = (s.remember(Some(MAC)), s.remember(Some(MAC)));
    s.devices.clear_wake_hints(&s.platform).unwrap();
    for id in [a, b] {
        let err = s.devices.wake(&s.platform, &id).unwrap_err();
        assert!(matches!(err, CoreError::NoWakeHint), "{err}");
    }
    assert!(
        s.devices
            .list(&s.platform)
            .unwrap()
            .iter()
            .all(|d| d.mac_hint.is_none())
    );
}

#[test]
fn wake_sends_magic_packet_for_hint() {
    let s = Store::new();
    let id = s.remember(Some(MAC));
    s.devices.wake(&s.platform, &id).unwrap();
    assert_eq!(s.platform.magic_packets(), [MAC]);
}

/// Every event of the process, from any thread, at debug level. The only
/// global-subscriber user in the devices test binary.
fn log() -> &'static Mutex<Vec<u8>> {
    static LOG: OnceLock<Arc<Mutex<Vec<u8>>>> = OnceLock::new();
    #[derive(Clone)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl Write for Buf {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    LOG.get_or_init(|| {
        let buf = Arc::new(Mutex::new(Vec::new()));
        let writer = Buf(buf.clone());
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .with_ansi(false)
            .with_writer(move || writer.clone())
            .finish();
        tracing::subscriber::set_global_default(subscriber).unwrap();
        buf
    })
}

#[test]
fn mac_never_logged() {
    let log = log();
    let sender = on_lan(Side::sender());
    let sender = Side {
        engine: sender.engine.clone().with_wol_hints(true),
        ..sender
    };
    moved(&sender, &Side::new());
    let [record] = known(&sender).try_into().unwrap();
    KnownDevices::open(&*sender.platform)
        .unwrap()
        .wake(&*sender.platform, &record.public)
        .unwrap();
    assert_eq!(sender.platform.magic_packets(), [MAC]);

    let text = String::from_utf8(log.lock().unwrap().clone()).unwrap();
    assert!(text.contains("wake packet"), "capture works");
    let forms = [
        MAC.map(|b| format!("{b:02x}")).join(":"),
        MAC.map(|b| format!("{b:02X}")).join(":"),
        MAC.map(|b| format!("{b:02x}")).join("-"),
        MAC.map(|b| format!("{b:02x}")).concat(),
        format!("{MAC:?}"),
    ];
    for form in forms {
        assert!(
            !text.to_lowercase().contains(&form.to_lowercase()),
            "{form}"
        );
    }
}
