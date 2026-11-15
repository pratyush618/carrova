//! Where a received file goes: the OS's own Downloads sink where apps may
//! not write paths (Android), else a hidden temp file in
//! [`Platform::downloads_dir`] renamed into place under a name that never
//! replaces an existing file (`report (2).txt`).
//!
//! Nothing is created before the first byte (or the commit of an empty
//! file), so a declined or expired offer leaves no trace.
//!
//! shortcut: an aborted transfer's partial file is deleted, so a retry
//! starts from byte 0; keep it per offer id for the link session once the
//! sender retries under the same id.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, OnceLock};

use carrova_link::{FileOffer, FileSink};
use carrova_platform::{DownloadSink, Platform, PlatformError};

use super::features::clean_mime;
use crate::error::CoreError;
use crate::fsx;

/// Names tried after the offered one: `name (2)` … `name (MAX_COPIES)`.
const MAX_COPIES: u32 = 999;

/// Temp files made by this process so far (part of their names).
static TEMPS: AtomicU64 = AtomicU64::new(0);

/// A received file on its way into Downloads.
#[derive(Debug)]
pub(super) struct Download {
    platform: Arc<dyn Platform>,
    /// The offer, once read (its name already safe:
    /// [`carrova_link::safe_file_name`]).
    offer: Arc<OnceLock<FileOffer>>,
    out: Option<Out>,
}

#[derive(Debug)]
enum Out {
    /// The OS saves it.
    Os(Box<dyn DownloadSink>),
    /// A hidden temp file in `dir`.
    Temp {
        file: File,
        dir: PathBuf,
        path: PathBuf,
    },
}

impl Download {
    /// The file `offer` will describe (set once it is read).
    pub(super) fn new(platform: Arc<dyn Platform>, offer: Arc<OnceLock<FileOffer>>) -> Self {
        Self {
            platform,
            offer,
            out: None,
        }
    }

    fn name(&self) -> io::Result<&str> {
        self.offer
            .get()
            .map(|o| o.name.as_str())
            .ok_or_else(|| io::Error::other("no offer"))
    }

    /// The open output, made on first need.
    fn out(&mut self) -> io::Result<&mut Out> {
        if self.out.is_none() {
            self.out = Some(self.open().map_err(io::Error::other)?);
        }
        self.out
            .as_mut()
            .ok_or_else(|| io::Error::other("no output"))
    }

    fn open(&self) -> Result<Out, CoreError> {
        let p = &*self.platform;
        let mime = clean_mime(self.offer.get().and_then(|o| o.mime.as_deref()));
        let mime = mime.as_deref().unwrap_or("application/octet-stream");
        match p.downloads_sink(self.name()?, mime) {
            Ok(sink) => return Ok(Out::Os(sink)),
            Err(PlatformError::Unsupported(_)) => {}
            Err(e) => return Err(e.into()),
        }
        let dir = p.downloads_dir()?;
        fsx::create_dir_or_existing(p, &dir, 0o700)?;
        let n = TEMPS.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!(".carrova-{}-{n}.part", std::process::id()));
        let file = p.create_new_file(&path, Some(0o600))?;
        Ok(Out::Temp { file, dir, path })
    }
}

impl FileSink for Download {
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
        let platform = Arc::clone(&self.platform);
        match self.out()? {
            Out::Os(sink) => sink.write_at(offset, bytes),
            Out::Temp { file, .. } => platform.write_all_at(file, bytes, offset),
        }
    }

    fn commit(mut self: Box<Self>) -> io::Result<PathBuf> {
        self.out()?;
        let name = self.name()?.to_owned();
        match self.out.take() {
            Some(Out::Os(sink)) => sink.commit().map_err(io::Error::other),
            Some(Out::Temp { file, dir, path }) => {
                let p = &*self.platform;
                let published = file
                    .sync_all()
                    .and_then(|()| publish(p, &path, &dir, &name));
                if published.is_err() {
                    remove(p, &path);
                }
                published
            }
            None => Err(io::Error::other("no output")),
        }
    }

    fn abort(mut self: Box<Self>) {
        match self.out.take() {
            Some(Out::Os(sink)) => sink.abort(),
            Some(Out::Temp { path, .. }) => remove(&*self.platform, &path),
            None => {}
        }
    }
}

/// Rename `temp` into `dir` as `name`, or the first free `name (n)`.
fn publish(p: &dyn Platform, temp: &Path, dir: &Path, name: &str) -> io::Result<PathBuf> {
    for copy in 1..=MAX_COPIES {
        let to = dir.join(numbered(name, copy));
        match p.rename_noreplace(temp, &to).map_err(CoreError::from) {
            Ok(()) => return Ok(to),
            Err(e) if fsx::kind_of(&e) == Some(io::ErrorKind::AlreadyExists) => {}
            Err(e) => return Err(io::Error::other(e)),
        }
    }
    Err(io::ErrorKind::AlreadyExists.into())
}

/// `name` for the first copy, then `stem (n).ext`.
fn numbered(name: &str, copy: u32) -> String {
    if copy == 1 {
        return name.to_owned();
    }
    // A leading dot starts a hidden name, not an extension.
    match name.rfind('.').filter(|&i| i > 0) {
        Some(i) => format!("{} ({copy}){}", &name[..i], &name[i..]),
        None => format!("{name} ({copy})"),
    }
}

fn remove(p: &dyn Platform, path: &Path) {
    if let Err(e) = p.remove_file(path) {
        tracing::warn!(error = %e, "could not remove a partial download");
    }
}

#[cfg(test)]
#[path = "../../tests/ecosystem/sink/unit.rs"]
mod tests;
