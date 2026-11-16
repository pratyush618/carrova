//! Files through `Files.kt`: the app's files directory, Downloads (written
//! through MediaStore, the only way apps may add files there on every
//! supported Android) and opening URLs and files with the user's default
//! app.

use std::fs::File;
use std::io;
use std::path::{Path, PathBuf};

use ::jni::JNIEnv;
use ::jni::objects::{JString, JValueOwned};

use super::jni;
use crate::{DownloadSink, PlatformError};

const TO_STRING: &str = "(Landroid/content/Context;)Ljava/lang/String;";
const STRING_TO_STRING: &str = "(Landroid/content/Context;Ljava/lang/String;)Ljava/lang/String;";
const STRING_TO_VOID: &str = "(Landroid/content/Context;Ljava/lang/String;)V";
const STRING_TO_INT: &str = "(Landroid/content/Context;Ljava/lang/String;)I";
const CREATE: &str =
    "(Landroid/content/Context;Ljava/lang/String;Ljava/lang/String;)Ljava/lang/String;";

/// `Context.getFilesDir()`.
pub(super) fn files_dir() -> Result<PathBuf, PlatformError> {
    jni::call("Files.filesDir", |env, context| {
        let dir = jni::call_static(
            env,
            context,
            "Files",
            "filesDir",
            TO_STRING,
            &[context.into()],
        )?;
        string(env, dir)
    })
    .map(PathBuf::from)
}

/// The shared Downloads directory.
pub(super) fn downloads_dir() -> Result<PathBuf, PlatformError> {
    jni::call("Files.downloadsDir", |env, context| {
        let dir = jni::call_static(
            env,
            context,
            "Files",
            "downloadsDir",
            TO_STRING,
            &[context.into()],
        )?;
        string(env, dir)
    })
    .map(PathBuf::from)
}

/// Open `url` (already checked `http(s)`) in the user's browser.
pub(super) fn open_url(url: &str) -> Result<(), PlatformError> {
    with_arg("Files.openUrl", "openUrl", STRING_TO_VOID, url, |_, v| {
        v.v()
    })
}

/// Open `path` (already checked absolute) with the user's default app.
pub(super) fn open_path(path: &Path) -> Result<(), PlatformError> {
    let path = path
        .to_str()
        .ok_or(PlatformError::InvalidInput("path is not UTF-8"))?;
    with_arg(
        "Files.openPath",
        "openPath",
        STRING_TO_VOID,
        path,
        |_, v| v.v(),
    )
}

/// A pending MediaStore Downloads entry, written through its descriptor;
/// dropped unpublished, the entry is deleted.
#[derive(Debug)]
pub(super) struct MediaStoreSink {
    file: Option<File>,
    /// `content://` URI of the entry (holds no file name).
    uri: String,
    published: bool,
}

impl MediaStoreSink {
    /// A new pending entry named like `name`.
    pub(super) fn create(name: &str, mime: &str) -> Result<Self, PlatformError> {
        let uri = jni::call("Files.create", |env, context| {
            let (name, mime) = (env.new_string(name)?, env.new_string(mime)?);
            let uri = jni::call_static(
                env,
                context,
                "Files",
                "create",
                CREATE,
                &[context.into(), (&name).into(), (&mime).into()],
            )?;
            string(env, uri)
        })?;
        let mut sink = Self {
            file: None,
            uri,
            published: false,
        };
        let fd = with_arg("Files.open", "open", STRING_TO_INT, &sink.uri, |_, v| v.i())?;
        sink.file = Some(jni::file_from_fd(fd).map_err(|e| PlatformError::io("Downloads", e))?);
        Ok(sink)
    }
}

impl DownloadSink for MediaStoreSink {
    fn write_at(&mut self, offset: u64, bytes: &[u8]) -> io::Result<()> {
        let file = self.file.as_ref().ok_or(io::ErrorKind::NotConnected)?;
        crate::sys::write_all_at(file, bytes, offset)
    }

    fn commit(mut self: Box<Self>) -> Result<PathBuf, PlatformError> {
        if let Some(file) = self.file.take() {
            file.sync_all()
                .map_err(|e| PlatformError::io("Downloads", e))?;
        }
        let name = with_arg(
            "Files.publish",
            "publish",
            STRING_TO_STRING,
            &self.uri,
            string,
        )?;
        self.published = true;
        Ok(downloads_dir()?.join(name))
    }

    fn abort(self: Box<Self>) {}
}

impl Drop for MediaStoreSink {
    fn drop(&mut self) {
        drop(self.file.take());
        if !self.published {
            // Nothing to report to: the transfer already failed.
            let _ = with_arg(
                "Files.discard",
                "discard",
                STRING_TO_VOID,
                &self.uri,
                |_, v| v.v(),
            );
        }
    }
}

/// `Files.<method>(context, arg)` (JNI signature `sig`), its result read
/// by `read`.
fn with_arg<T>(
    call: &'static str,
    method: &str,
    sig: &str,
    arg: &str,
    read: impl FnOnce(&mut JNIEnv<'_>, JValueOwned<'_>) -> ::jni::errors::Result<T>,
) -> Result<T, PlatformError> {
    jni::call(call, |env, context| {
        let arg = env.new_string(arg)?;
        let value = jni::call_static(
            env,
            context,
            "Files",
            method,
            sig,
            &[context.into(), (&arg).into()],
        )?;
        read(env, value)
    })
}

pub(super) fn string(
    env: &mut JNIEnv<'_>,
    value: JValueOwned<'_>,
) -> ::jni::errors::Result<String> {
    Ok(env.get_string(&JString::from(value.l()?))?.into())
}
