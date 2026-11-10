//! Real `gpg-keyring` round trip between two temp GNUPGHOMEs. Skipped when
//! `gpg` is not on PATH.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/apply.rs"]
mod apply_support;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;

use std::path::{Path, PathBuf};
use std::process::Command;

use apply_support::{Env, plan};
use carrova_core::plan::PlannedHandler;
use carrova_core::session::NullSink;
use carrova_core::test_support::apply::{Failpoint, apply};
use carrova_core::test_support::handlers;
use carrova_core::test_support::rollback::rollback;
use carrova_platform::SemanticRoot::Home;

fn find_on_path(name: &str) -> Option<PathBuf> {
    carrova_platform::current().find_binary(name).ok().flatten()
}

fn gpg_in(bin: &Path, gnupghome: &Path) -> Command {
    let mut c = Command::new(bin);
    c.env("GNUPGHOME", gnupghome).args(["--batch", "--no-tty"]);
    c
}

/// Stops agents started for a temp GNUPGHOME.
struct KillAgents(Vec<PathBuf>);
impl Drop for KillAgents {
    fn drop(&mut self) {
        if let Some(gpgconf) = find_on_path("gpgconf") {
            for h in &self.0 {
                let _ = Command::new(&gpgconf)
                    .env("GNUPGHOME", h)
                    .args(["--kill", "all"])
                    .output();
            }
        }
    }
}

#[test]
fn gpg_keyring_round_trip() {
    let Some(gpg) = find_on_path("gpg") else {
        eprintln!("gpg not on PATH; skipping");
        return;
    };
    let src = Env::with(|p| p.binary("gpg", &gpg));
    let mut dst = Env::with(|p| p.binary("gpg", &gpg));
    let src_gh = src.home.join(".gnupg");
    let dst_gh = dst.home.join(".gnupg");
    let _guard = KillAgents(vec![src_gh.clone(), dst_gh.clone()]);
    std::fs::create_dir(&src_gh).unwrap();
    carrova_platform::Platform::set_mode(&src.platform, &src_gh, 0o700).unwrap();

    let out = gpg_in(&gpg, &src_gh)
        .args([
            "--pinentry-mode",
            "loopback",
            "--passphrase",
            "",
            "--quick-gen-key",
            "Carrova Test <carrova-test@example.invalid>",
            "ed25519",
            "default",
            "never",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "keygen failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let arts = handlers::export(&src.platform, "gpg-keyring").unwrap();
    let names: Vec<&str> = arts.iter().map(|a| a.name.as_str()).collect();
    assert_eq!(names, ["pubring.asc", "ownertrust.txt"]);
    assert!(
        arts[0]
            .bytes
            .starts_with(b"-----BEGIN PGP PUBLIC KEY BLOCK-----")
    );
    for a in &arts {
        dst.stage_artifact("gnupg/public-keyring", &a.name, &a.bytes);
    }

    let h = PlannedHandler {
        item: "gnupg/public-keyring".into(),
        handler: carrova_core::plan::Handler::GpgKeyring,
        backup: vec![
            (Home, ".gnupg/pubring.kbx".into()),
            (Home, ".gnupg/trustdb.gpg".into()),
            (Home, ".gnupg/public-keys.d".into()),
        ],
        import_trust: true,
    };
    apply(
        &dst.platform,
        &plan(vec![], vec![h]),
        &dst.manifest(),
        &dst.staged,
        &dst.session_dir,
        &NullSink,
        Failpoint::OFF,
    )
    .unwrap();
    assert_eq!(dst.mode(&dst_gh), 0o700);
    let list = gpg_in(&gpg, &dst_gh)
        .args(["--list-keys", "--with-colons"])
        .output()
        .unwrap();
    assert!(list.status.success());
    let listing = String::from_utf8_lossy(&list.stdout);
    assert!(
        listing.contains("carrova-test@example.invalid"),
        "key not imported"
    );
    // Ownertrust (ultimate for the generated key) came along.
    let trust = gpg_in(&gpg, &dst_gh)
        .arg("--export-ownertrust")
        .output()
        .unwrap();
    assert!(String::from_utf8_lossy(&trust.stdout).contains(":6:"));

    // Rollback clears the GNUPGHOME apply created (gpg's own leftovers and
    // the trustdb it rewrote while listing are quarantined, not deleted).
    let _ = Command::new(find_on_path("gpgconf").unwrap_or_default())
        .env("GNUPGHOME", &dst_gh)
        .args(["--kill", "all"])
        .output();
    let rb = rollback(&dst.platform, &dst.session_dir, &NullSink).unwrap();
    assert!(rb.failed.is_empty(), "{:?}", rb.failed);
    assert!(!dst_gh.exists());
    assert!(rb.quarantined.iter().all(|q| {
        q.moved_to
            .starts_with(dst.session_dir.join("rollback-trash"))
    }));
}
