//! The hand-over: check, permission, check again, install; a bad APK never
//! stays.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use carrova_bundle::app_update::{ApkRejected, AppUpdateError, Control, Release, install};
use tempfile::TempDir;

use crate::fakes::{FakePhone, Log, Server, content};

const SIZE: usize = 64 * 1024;

fn release() -> Release {
    Release::parse(br#"{"version":"0.4.0"}"#).unwrap()
}

/// Serves the release's real APK address.
struct Releases(Server);

impl carrova_bundle::app_update::Get for Releases {
    fn get(
        &self,
        url: &str,
        resume: Option<&carrova_bundle::app_update::Resume>,
    ) -> Result<carrova_bundle::app_update::Reply, carrova_bundle::app_update::NetFailure> {
        let url = if url == release().apk_url() {
            crate::fakes::URL
        } else {
            url
        };
        self.0.get(url, resume)
    }
}

fn run(dir: &Path, phone: &FakePhone, log: &Log) -> Result<(), AppUpdateError> {
    let server = Releases(Server::new(content(SIZE, 0)));
    install(
        &server,
        phone,
        log,
        &Control::default(),
        &release(),
        dir,
        false,
    )
}

fn apk(dir: &Path) -> PathBuf {
    dir.join(release().apk_name())
}

#[test]
fn a_checked_apk_goes_to_the_installer() {
    let dir = TempDir::new().unwrap();
    let (phone, log) = (FakePhone::new(true), Log::default());
    run(dir.path(), &phone, &log).unwrap();
    assert_eq!(*phone.installed_apks.lock().unwrap(), [apk(dir.path())]);
    assert_eq!(
        log.names(),
        ["fetching", "received", "verifying", "installing"]
    );
}

#[test]
fn an_unreadable_apk_is_deleted_so_the_next_try_downloads_again() {
    let dir = TempDir::new().unwrap();
    let phone = FakePhone {
        unreadable: true,
        ..FakePhone::new(true)
    };
    let err = run(dir.path(), &phone, &Log::default()).unwrap_err();
    assert!(matches!(err, AppUpdateError::Platform(_)));
    assert!(!apk(dir.path()).exists());
    assert!(phone.installed_apks.lock().unwrap().is_empty());
}

#[test]
fn an_apk_signed_with_another_key_is_deleted() {
    let dir = TempDir::new().unwrap();
    let phone = FakePhone {
        signer: [7; 32],
        ..FakePhone::new(true)
    };
    let err = run(dir.path(), &phone, &Log::default()).unwrap_err();
    assert!(matches!(
        err,
        AppUpdateError::Rejected(ApkRejected::WrongSigner)
    ));
    assert!(!apk(dir.path()).exists());
}

#[test]
fn it_waits_for_the_permission_then_checks_again_before_installing() {
    let dir = TempDir::new().unwrap();
    let (phone, log) = (FakePhone::new(true), Log::default());
    phone.may_install.store(false, Ordering::SeqCst);
    let err = thread::scope(|s| {
        s.spawn(|| {
            // Answer only once the install waits (bounded, so a run that
            // never asks fails the asserts below instead of hanging).
            for _ in 0..1000 {
                if log.names().contains(&"permission") {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
            // Meanwhile the same build got installed another way.
            phone.installed_code.store(4000, Ordering::SeqCst);
            phone.may_install.store(true, Ordering::SeqCst);
        });
        run(dir.path(), &phone, &log).unwrap_err()
    });
    assert!(matches!(
        err,
        AppUpdateError::Rejected(ApkRejected::NotNewer)
    ));
    assert!(log.names().contains(&"permission"));
    assert!(phone.installed_apks.lock().unwrap().is_empty());
    assert!(!apk(dir.path()).exists());
}

#[test]
fn a_downloaded_apk_is_not_fetched_again() {
    let dir = TempDir::new().unwrap();
    std::fs::write(apk(dir.path()), b"apk").unwrap();
    let log = Log::default();
    run(dir.path(), &FakePhone::new(true), &log).unwrap();
    assert_eq!(log.names(), ["verifying", "installing"]);
}

#[test]
fn other_versions_downloads_go_but_folders_stay() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("Carrova_0.3.9_android-arm64.apk"), b"old").unwrap();
    std::fs::create_dir(dir.path().join("keep")).unwrap();
    run(dir.path(), &FakePhone::new(true), &Log::default()).unwrap();
    assert!(!dir.path().join("Carrova_0.3.9_android-arm64.apk").exists());
    assert!(dir.path().join("keep").is_dir());
}
