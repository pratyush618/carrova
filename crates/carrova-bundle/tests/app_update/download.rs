//! The resumable download: resume only into the same file, pause, Wi-Fi,
//! cancel, size cap, trusted redirects.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::thread;
use std::time::Duration;

use carrova_bundle::app_update::{
    AppUpdateError, Control, Job, NetFailure, download, fetch_release,
};
use carrova_platform::UpdateTap;
use tempfile::TempDir;

use crate::fakes::{ASSET, FakePhone, Log, Server, URL, content};

const SIZE: usize = 300 * 1024;

struct Run {
    dir: TempDir,
    log: Log,
    control: Arc<Control>,
    max_size: u64,
}

impl Run {
    fn new() -> Self {
        Self {
            dir: TempDir::new().unwrap(),
            log: Log::default(),
            control: Arc::default(),
            max_size: 10 * SIZE as u64,
        }
    }

    fn part(&self) -> PathBuf {
        self.dir.path().join("x.apk.part")
    }

    fn etag(&self) -> PathBuf {
        self.dir.path().join("x.apk.part.etag")
    }

    fn go(
        &self,
        server: &Server,
        phone: &FakePhone,
        mobile_data: bool,
    ) -> Result<(), AppUpdateError> {
        let part = self.part();
        let job = Job {
            url: URL,
            part: &part,
            mobile_data,
            max_size: self.max_size,
        };
        download(server, phone, &self.log, &self.control, job)
    }

    /// A partial download left by an earlier run of the server's `etag`.
    fn left(&self, bytes: &[u8], etag: &str) {
        std::fs::write(self.part(), bytes).unwrap();
        std::fs::write(self.etag(), etag).unwrap();
    }
}

#[test]
fn downloads_the_whole_file_through_the_trusted_redirect() {
    let run = Run::new();
    let server = Server::new(content(SIZE, 0));
    run.go(&server, &FakePhone::new(true), false).unwrap();
    assert_eq!(std::fs::read(run.part()).unwrap(), content(SIZE, 0));
    assert_eq!(std::fs::read_to_string(run.etag()).unwrap(), "\"v1\"");
    assert_eq!(run.log.names(), ["fetching", "received"]);
}

#[test]
fn resumes_the_same_file_from_the_partial_download() {
    let run = Run::new();
    run.left(&content(SIZE, 0)[..1000], "\"v1\"");
    let server = Server::new(content(SIZE, 0));
    run.go(&server, &FakePhone::new(true), false).unwrap();
    assert_eq!(server.offsets(), [Some(1000)]);
    assert_eq!(std::fs::read(run.part()).unwrap(), content(SIZE, 0));
}

#[test]
fn a_replaced_asset_starts_over_instead_of_joining_two_files() {
    let run = Run::new();
    run.left(&content(SIZE, 0)[..1000], "\"v1\"");
    let server = Server::new(content(SIZE, 0));
    server.replace(content(SIZE, 7), "\"v2\"");
    run.go(&server, &FakePhone::new(true), false).unwrap();
    assert_eq!(std::fs::read(run.part()).unwrap(), content(SIZE, 7));
    assert_eq!(std::fs::read_to_string(run.etag()).unwrap(), "\"v2\"");
}

#[test]
fn a_range_from_the_wrong_offset_starts_over() {
    let run = Run::new();
    run.left(&content(SIZE, 0)[..1000], "\"v1\"");
    let server = Server {
        wrong_start: true,
        ..Server::new(content(SIZE, 0))
    };
    run.go(&server, &FakePhone::new(true), false).unwrap();
    assert_eq!(server.offsets(), [Some(1000), None]);
    assert_eq!(std::fs::read(run.part()).unwrap(), content(SIZE, 0));
}

#[test]
fn a_partial_file_without_its_etag_starts_over() {
    let run = Run::new();
    std::fs::write(run.part(), b"stale").unwrap();
    let server = Server::new(content(SIZE, 0));
    run.go(&server, &FakePhone::new(true), false).unwrap();
    assert_eq!(server.offsets(), [None]);
    assert_eq!(std::fs::read(run.part()).unwrap(), content(SIZE, 0));
}

#[test]
fn a_redirect_off_github_is_refused() {
    let run = Run::new();
    let server = Server {
        redirect: "https://evil.example/x.apk".to_owned(),
        ..Server::new(content(SIZE, 0))
    };
    let err = run.go(&server, &FakePhone::new(true), false).unwrap_err();
    assert!(matches!(
        err,
        AppUpdateError::Network(NetFailure::Untrusted)
    ));
    assert_eq!(server.asked.lock().unwrap().len(), 1);
}

#[test]
fn the_feed_too_is_read_only_from_github() {
    let server = Server {
        redirect: "http://github.com/latest.json".to_owned(),
        ..Server::new(Vec::new())
    };
    // The feed URL itself redirects (to the fake's asset), here off HTTPS.
    let err = fetch_release(&FeedVia(&server)).unwrap_err();
    assert!(matches!(
        err,
        AppUpdateError::Network(NetFailure::Untrusted)
    ));
}

/// Answers the feed URL with the server's redirect.
struct FeedVia<'a>(&'a Server);

impl carrova_bundle::app_update::Get for FeedVia<'_> {
    fn get(
        &self,
        url: &str,
        resume: Option<&carrova_bundle::app_update::Resume>,
    ) -> Result<carrova_bundle::app_update::Reply, NetFailure> {
        self.0.get(
            if url == carrova_bundle::app_update::FEED_URL {
                URL
            } else {
                url
            },
            resume,
        )
    }
}

#[test]
fn errors_never_carry_the_signed_download_link() {
    let run = Run::new();
    let server = Server::new(content(SIZE, 0));
    *server.breaks_after.lock().unwrap() = Some(1000);
    let err = run.go(&server, &FakePhone::new(true), false).unwrap_err();
    assert!(!err.to_string().contains("token"), "{err}");
    assert!(!err.to_string().contains(ASSET), "{err}");
}

#[test]
fn a_file_larger_than_an_update_can_be_is_refused() {
    let mut run = Run::new();
    run.max_size = SIZE as u64 - 1;
    let err = run
        .go(&Server::new(content(SIZE, 0)), &FakePhone::new(true), false)
        .unwrap_err();
    assert!(matches!(err, AppUpdateError::TooLarge));
    assert!(!run.part().exists());
}

#[test]
fn the_size_cap_holds_without_a_content_length() {
    let mut run = Run::new();
    run.max_size = SIZE as u64 - 1;
    let server = Server {
        no_length: true,
        ..Server::new(content(SIZE, 0))
    };
    let err = run.go(&server, &FakePhone::new(true), false).unwrap_err();
    assert!(matches!(err, AppUpdateError::TooLarge));
    assert!(!run.part().exists());
}

#[test]
fn off_wifi_it_waits_until_mobile_data_is_allowed() {
    let run = Run::new();
    let control = Arc::clone(&run.control);
    let allow = thread::spawn(move || {
        thread::sleep(Duration::from_millis(100));
        control.update(|wish| wish.mobile_data = true);
    });
    run.go(
        &Server::new(content(SIZE, 0)),
        &FakePhone::new(false),
        false,
    )
    .unwrap();
    allow.join().unwrap();
    assert_eq!(run.log.names(), ["waiting", "fetching", "received"]);
}

#[test]
fn mobile_data_allowed_in_settings_downloads_off_wifi() {
    let run = Run::new();
    run.go(&Server::new(content(SIZE, 0)), &FakePhone::new(false), true)
        .unwrap();
    assert_eq!(run.log.names(), ["fetching", "received"]);
}

#[test]
fn wifi_dropping_mid_download_waits_then_resumes_where_it_stopped() {
    let run = Run::new();
    let phone = FakePhone::new(true);
    let server = Server {
        drops: Some(Arc::clone(&phone.wifi)),
        ..Server::new(content(SIZE, 0))
    };
    *server.breaks_after.lock().unwrap() = Some(1000);
    let wifi = Arc::clone(&phone.wifi);
    let back = thread::spawn(move || {
        while wifi.load(Ordering::SeqCst) {
            thread::yield_now();
        }
        thread::sleep(Duration::from_millis(100));
        wifi.store(true, Ordering::SeqCst);
    });
    run.go(&server, &phone, false).unwrap();
    back.join().unwrap();
    assert_eq!(
        run.log.names(),
        ["fetching", "received", "waiting", "fetching", "received"]
    );
    assert_eq!(server.offsets(), [None, Some(1000)]);
    assert_eq!(std::fs::read(run.part()).unwrap(), content(SIZE, 0));
}

#[test]
fn a_notification_tap_pauses_and_resumes() {
    let run = Run::new();
    let phone = FakePhone::new(true);
    phone
        .taps
        .lock()
        .unwrap()
        .extend([UpdateTap::Pause, UpdateTap::Resume]);
    run.go(&Server::new(content(SIZE, 0)), &phone, false)
        .unwrap();
    assert_eq!(run.log.names(), ["paused", "fetching", "received"]);
}

#[test]
fn cancelling_deletes_the_partial_file() {
    let run = Run::new();
    run.left(b"half", "\"v1\"");
    run.control.update(|wish| wish.cancelled = true);
    let err = run
        .go(&Server::new(content(SIZE, 0)), &FakePhone::new(true), false)
        .unwrap_err();
    assert!(matches!(err, AppUpdateError::Cancelled));
    assert!(!run.part().exists());
    assert!(!run.etag().exists());
}

#[test]
fn a_broken_connection_fails_and_keeps_what_arrived() {
    let run = Run::new();
    let server = Server::new(content(SIZE, 0));
    *server.breaks_after.lock().unwrap() = Some(1000);
    let err = run.go(&server, &FakePhone::new(true), false).unwrap_err();
    assert!(matches!(err, AppUpdateError::Network(_)));
    assert_eq!(
        std::fs::read(run.part()).unwrap(),
        &content(SIZE, 0)[..1000]
    );
}

#[test]
fn a_second_update_is_refused_while_one_runs_and_keeps_its_cancel() {
    let control = Control::default();
    let first = control.begin(false).unwrap();
    control.update(|wish| wish.cancelled = true);
    assert!(matches!(control.begin(true), Err(AppUpdateError::Busy)));
    assert!(control.wish().cancelled);
    drop(first);
    let _again = control.begin(true).unwrap();
    assert!(!control.wish().cancelled);
    assert!(control.wish().mobile_data);
}
