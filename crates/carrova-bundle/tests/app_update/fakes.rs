//! A release server, a phone and a progress log for tests.

use std::collections::VecDeque;
use std::io::{self, Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use carrova_bundle::app_update::{
    Get, NetFailure, Phone, Progress, RELEASE_CERT_SHA256, Reply, Resume, Step,
};
use carrova_platform::{AppPackage, PlatformError, UpdateTap};

/// Where the fake release APK lives.
pub const URL: &str = "https://github.com/pratyush618/carrova/releases/download/v0.4.0/x.apk";
/// Where GitHub redirects it.
pub const ASSET: &str = "https://release-assets.githubusercontent.com/x.apk?token=secret";

/// The APK's bytes: every byte tells where it is.
pub fn content(size: usize, seed: u8) -> Vec<u8> {
    (0..size).map(|i| (i % 251) as u8 ^ seed).collect()
}

/// Serves one file like GitHub: a redirect to asset storage, then the file
/// with an `ETag`, honouring `Range` only with a matching `If-Range`.
pub struct Server {
    pub file: Mutex<Vec<u8>>,
    pub etag: Mutex<String>,
    /// Where the first hop redirects.
    pub redirect: String,
    /// Answer a range from the wrong offset.
    pub wrong_start: bool,
    /// Send no `Content-Length`.
    pub no_length: bool,
    /// Answer a range with only this many bytes, the file's size unsaid
    /// (`Content-Range: bytes 4-7/*`).
    pub short_range: Option<usize>,
    /// Break the next body after this many bytes.
    pub breaks_after: Mutex<Option<usize>>,
    /// Wi-Fi drops as the body breaks.
    pub drops: Option<Arc<AtomicBool>>,
    /// Every GET: its URL and the offset resumed from.
    pub asked: Mutex<Vec<(String, Option<u64>)>>,
}

impl Server {
    pub fn new(file: Vec<u8>) -> Self {
        Self {
            file: Mutex::new(file),
            etag: Mutex::new("\"v1\"".to_owned()),
            redirect: ASSET.to_owned(),
            wrong_start: false,
            no_length: false,
            short_range: None,
            breaks_after: Mutex::new(None),
            drops: None,
            asked: Mutex::default(),
        }
    }

    /// The file changes on the server (re-uploaded asset).
    pub fn replace(&self, file: Vec<u8>, etag: &str) {
        *self.file.lock().unwrap() = file;
        *self.etag.lock().unwrap() = etag.to_owned();
    }

    pub fn offsets(&self) -> Vec<Option<u64>> {
        self.asked
            .lock()
            .unwrap()
            .iter()
            .filter(|(url, _)| url != URL)
            .map(|(_, offset)| *offset)
            .collect()
    }
}

/// A reader that fails after its bytes, taking Wi-Fi with it.
struct Breaking(Cursor<Vec<u8>>, Option<Arc<AtomicBool>>);

impl Read for Breaking {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.0.read(buf)? {
            0 => {
                if let Some(wifi) = &self.1 {
                    wifi.store(false, Ordering::SeqCst);
                }
                Err(io::ErrorKind::ConnectionReset.into())
            }
            n => Ok(n),
        }
    }
}

fn reply(status: u16) -> Reply {
    Reply {
        status,
        location: None,
        etag: None,
        content_range: None,
        content_length: None,
        body: Box::new(io::empty()),
    }
}

impl Get for Server {
    fn get(&self, url: &str, resume: Option<&Resume>) -> Result<Reply, NetFailure> {
        self.asked
            .lock()
            .unwrap()
            .push((url.to_owned(), resume.map(|r| r.offset)));
        if url == URL {
            return Ok(Reply {
                location: Some(self.redirect.clone()),
                ..reply(302)
            });
        }
        let file = self.file.lock().unwrap().clone();
        let etag = self.etag.lock().unwrap().clone();
        let size = file.len() as u64;
        let ranged = resume.filter(|r| r.etag == etag);
        let (status, start) = match ranged {
            Some(r) if r.offset >= size => return Ok(reply(416)),
            Some(r) => (206, if self.wrong_start { 0 } else { r.offset }),
            None => (200, 0),
        };
        let mut rest = file[start as usize..].to_vec();
        let short = self.short_range.filter(|_| status == 206);
        if let Some(n) = short {
            rest.truncate(n);
        }
        let length = rest.len() as u64;
        let body: Box<dyn Read + Send> = match self.breaks_after.lock().unwrap().take() {
            Some(n) => {
                rest.truncate(n);
                Box::new(Breaking(Cursor::new(rest), self.drops.clone()))
            }
            None => Box::new(Cursor::new(rest)),
        };
        Ok(Reply {
            status,
            location: None,
            etag: Some(etag),
            content_range: (status == 206).then(|| match short {
                Some(_) => format!("bytes {start}-{}/*", start + length - 1),
                None => format!("bytes {start}-{}/{size}", size - 1),
            }),
            content_length: (!self.no_length).then_some(length),
            body,
        })
    }
}

/// A phone on Wi-Fi or not, with taps queued by the test, its install
/// permission and the packages it reads.
pub struct FakePhone {
    pub wifi: Arc<AtomicBool>,
    pub taps: Mutex<VecDeque<UpdateTap>>,
    pub may_install: Arc<AtomicBool>,
    /// The APK's build, and the installed one.
    pub apk_code: u64,
    pub installed_code: Arc<AtomicU64>,
    pub signer: [u8; 32],
    /// Reading the APK fails.
    pub unreadable: bool,
    pub inspected: Mutex<Vec<PathBuf>>,
    pub installed_apks: Mutex<Vec<PathBuf>>,
}

impl FakePhone {
    pub fn new(wifi: bool) -> Self {
        Self {
            wifi: Arc::new(AtomicBool::new(wifi)),
            taps: Mutex::default(),
            may_install: Arc::new(AtomicBool::new(true)),
            apk_code: 4000,
            installed_code: Arc::new(AtomicU64::new(3000)),
            signer: RELEASE_CERT_SHA256,
            unreadable: false,
            inspected: Mutex::default(),
            installed_apks: Mutex::default(),
        }
    }

    fn package(code: u64, signer: [u8; 32]) -> AppPackage {
        AppPackage {
            package: "org.byteveda.Carrova".to_owned(),
            version_code: code,
            signers: vec![signer],
        }
    }
}

impl Phone for FakePhone {
    fn on_wifi(&self) -> bool {
        self.wifi.load(Ordering::SeqCst)
    }

    fn tap(&self) -> Option<UpdateTap> {
        self.taps.lock().unwrap().pop_front()
    }

    fn inspect(&self, apk: &Path) -> Result<AppPackage, PlatformError> {
        self.inspected.lock().unwrap().push(apk.to_owned());
        if self.unreadable {
            return Err(PlatformError::Android {
                call: "Updates.inspect",
                cause: "IllegalStateException".to_owned(),
            });
        }
        Ok(Self::package(self.apk_code, self.signer))
    }

    fn installed(&self) -> Result<AppPackage, PlatformError> {
        Ok(Self::package(
            self.installed_code.load(Ordering::SeqCst),
            RELEASE_CERT_SHA256,
        ))
    }

    fn can_install(&self) -> Result<bool, PlatformError> {
        Ok(self.may_install.load(Ordering::SeqCst))
    }

    fn install(&self, apk: &Path) -> Result<(), PlatformError> {
        self.installed_apks.lock().unwrap().push(apk.to_owned());
        Ok(())
    }
}

/// Records every step.
#[derive(Default)]
pub struct Log(pub Mutex<Vec<Step>>);

impl Progress for Log {
    fn step(&self, step: Step) {
        self.0.lock().unwrap().push(step);
    }
}

impl Log {
    /// Step names in order, repeats folded.
    pub fn names(&self) -> Vec<&'static str> {
        let mut names: Vec<_> = self
            .0
            .lock()
            .unwrap()
            .iter()
            .map(|step| match step {
                Step::Fetching { .. } => "fetching",
                Step::Received { .. } => "received",
                Step::Held { stage, .. } => match stage {
                    carrova_platform::UpdateStage::Paused => "paused",
                    _ => "waiting",
                },
                Step::Verifying => "verifying",
                Step::NeedsPermission => "permission",
                Step::Installing => "installing",
            })
            .collect();
        names.dedup();
        names
    }

    /// Returns once step `name` is logged, or after ten seconds, so a run
    /// that never gets there fails its asserts instead of hanging.
    pub fn wait_for(&self, name: &str) {
        for _ in 0..1000 {
            if self.names().contains(&name) {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}
