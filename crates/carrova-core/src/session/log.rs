//! Per-session event log (`log.jsonl`) and its redacted export.
//!
//! [`LogSink`] appends every [`Event`] as one JSON line (`{"ts": <unix
//! secs>, "event": ..., ...}`, file mode `600`) and forwards the event to the
//! front-end's sink. Events never carry file contents or secrets, so the log
//! holds paths and item keys only. A failing log write is reported through
//! `tracing` and never stops the session.
//!
//! [`redact_log`] produces the shareable copy: every string that contains
//! a path of a sensitive item (`<root>:<rel>` key or absolute path) is
//! replaced by `<redacted>` entirely, and the receiving user's home
//! directory prefix in any remaining string becomes `~`.

use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

use carrova_platform::{Platform, PlatformError};
use serde_json::Value;

use crate::error::{CoreError, Result};
use crate::session::{Event, EventSink};

/// Replacement for redacted strings.
pub const REDACTED: &str = "<redacted>";

/// Appends events to `log.jsonl` and forwards them.
pub struct LogSink {
    file: Mutex<Option<File>>,
    inner: Arc<dyn EventSink>,
}

impl std::fmt::Debug for LogSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LogSink").finish_non_exhaustive()
    }
}

impl LogSink {
    /// Open `path` for appending (created `600` if missing). An existing
    /// non-regular file (e.g. a symlink) is refused: the open is no-follow
    /// and checked with fstat, so a swapped-in link is never written
    /// through.
    pub fn open(platform: &dyn Platform, path: &Path, inner: Arc<dyn EventSink>) -> Result<Self> {
        let file = match platform.open_append_nofollow(path) {
            Ok(f) => f,
            Err(PlatformError::Io { source, .. }) if source.kind() == ErrorKind::NotFound => {
                platform.create_new_file(path, Some(0o600))?
            }
            Err(source) => {
                return Err(CoreError::LogOpen {
                    path: path.to_path_buf(),
                    source,
                });
            }
        };
        Ok(Self {
            file: Mutex::new(Some(file)),
            inner,
        })
    }

    /// A sink that only forwards (no log file).
    #[must_use]
    pub fn forward_only(inner: Arc<dyn EventSink>) -> Self {
        Self {
            file: Mutex::new(None),
            inner,
        }
    }

    fn append(&self, event: &Event) {
        let Ok(mut guard) = self.file.lock() else {
            return;
        };
        let Some(file) = guard.as_mut() else {
            return;
        };
        let mut value = match serde_json::to_value(event) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, "cannot encode event for session log");
                return;
            }
        };
        if let Value::Object(map) = &mut value {
            map.insert("ts".into(), Value::from(crate::time::now_unix()));
        }
        let mut line = value.to_string();
        line.push('\n');
        if let Err(e) = file.write_all(line.as_bytes()) {
            tracing::warn!(error = %e, "cannot append to session log");
        }
    }
}

impl EventSink for LogSink {
    fn emit(&self, event: Event) {
        self.append(&event);
        self.inner.emit(event);
    }
}

/// What to scrub from an exported log.
#[derive(Debug, Clone, Default)]
pub struct Redaction {
    /// Substrings marking a sensitive path (`<root>:<rel>` keys and
    /// absolute paths). A string containing any of them is replaced whole.
    pub sensitive: Vec<String>,
    /// Absolute home directory, replaced by `~`.
    pub home: Option<String>,
}

impl Redaction {
    fn scrub(&self, s: &str) -> String {
        if self
            .sensitive
            .iter()
            .any(|p| !p.is_empty() && s.contains(p.as_str()))
        {
            return REDACTED.to_owned();
        }
        match &self.home {
            Some(h) if !h.is_empty() && h != "/" => s.replace(h.as_str(), "~"),
            _ => s.to_owned(),
        }
    }

    fn value(&self, v: &mut Value) {
        match v {
            Value::String(s) => *s = self.scrub(s),
            Value::Array(a) => a.iter_mut().for_each(|v| self.value(v)),
            Value::Object(m) => m.values_mut().for_each(|v| self.value(v)),
            _ => {}
        }
    }
}

/// Redact a `log.jsonl` body line by line. Lines that are not valid JSON
/// (e.g. a torn last line) are dropped.
#[must_use]
pub fn redact_log(body: &[u8], redaction: &Redaction) -> String {
    let mut out = String::new();
    for line in body.split(|&b| b == b'\n') {
        let Ok(mut v) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        redaction.value(&mut v);
        out.push_str(&v.to_string());
        out.push('\n');
    }
    out
}

#[cfg(test)]
#[path = "../../tests/session/log/unit.rs"]
mod tests;
