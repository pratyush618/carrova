//! LAN migration throughput: two engines in one process over loopback, one
//! big file (default 256 MiB), homes under the system temp dir.
//!
//! ```text
//! cargo run --release -p carrova-core --example lan_bench [-- MIB]
//! ```
//!
//! Prints timings only; nothing is asserted.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::print_stdout)]

use std::io::Write;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use carrova_core::{
    CodeOffer, Engine, Exporter, Importer, NullSink, PlanOptions, Preset, ScanOptions, SendPrompt,
    Ticket,
};
use carrova_platform::fake::FakePlatform;

/// The code is handed over directly; no new code after a failed attempt.
struct Silent;

impl SendPrompt for Silent {
    fn show_code(&mut self, _: &Ticket, _: CodeOffer, _: Duration) {}

    fn confirm_new_code(&mut self, _: u32, _: u32) -> bool {
        false
    }
}

const RECIPE: &str = r"
schema: 2
id: app
name: The App
category: test
detect:
  linux:
    - binary: app
items:
  - id: data
    label: Data
    paths:
      - { root: user_data, rel: app }
    flags:
      default: true
";

fn engine(root: &Path) -> Engine {
    let home = root.join("home");
    std::fs::create_dir_all(home.join(".local/share")).unwrap();
    let platform = FakePlatform::with_home(&home)
        .with_state_dir(root.join("state"))
        .binary("app", "/usr/bin/app");
    Engine::new(
        Arc::new(platform),
        vec![carrova_recipe::load_str(RECIPE, Path::new("app/recipe.yaml")).unwrap()],
        Arc::new(NullSink),
    )
}

fn main() {
    let mib: usize = std::env::args()
        .nth(1)
        .map_or(256, |n| n.parse().expect("MIB must be a number"));
    let bytes = mib * 1024 * 1024;
    let tmp = tempfile::tempdir().unwrap();
    let sender = engine(&tmp.path().join("a"));
    let receiver = engine(&tmp.path().join("b"));

    let dir = tmp.path().join("a/home/.local/share/app");
    std::fs::create_dir_all(&dir).unwrap();
    let mut f = sender
        .platform()
        .create_new_file(&dir.join("big.bin"), Some(0o600))
        .unwrap();
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let mut block = vec![0u8; 1024 * 1024];
    for _ in 0..mib {
        for b in &mut block {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            *b = (x >> 24) as u8;
        }
        f.write_all(&block).unwrap();
    }
    f.sync_all().unwrap();
    drop(f);

    let started = Instant::now();
    let mut exp = Exporter::start(
        sender,
        ScanOptions {
            include_undetected: false,
            unrecognised: false,
        },
    )
    .unwrap();
    let sel = exp.suggest(Preset::Suggested);
    exp.select(sel).unwrap();
    let handle = exp.prepare_send().unwrap();
    let capture = started.elapsed();
    let port = handle.port();
    let ticket = Ticket {
        code: handle.ticket().unwrap().code.clone(),
        addr: Some(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)),
    };
    let sending = std::thread::spawn(move || handle.run(&mut Silent));

    let mut imp = Importer::connect(receiver, &ticket).unwrap();
    imp.plan(&PlanOptions::default()).unwrap();
    imp.approve().unwrap();
    let applying = Instant::now();
    imp.apply().unwrap();
    let apply = applying.elapsed();
    sending.join().unwrap().unwrap();

    let mb = bytes as f64 / 1e6;
    println!("payload {mib} MiB");
    println!(
        "sender capture: {capture:?} ({:.0} MB/s)",
        mb / capture.as_secs_f64()
    );
    println!(
        "receiver transfer + stage + apply: {apply:?} ({:.0} MB/s)",
        mb / apply.as_secs_f64()
    );
}
