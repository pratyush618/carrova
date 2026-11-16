use std::path::PathBuf;

/// Errors returned by [`crate::Platform`] implementations.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    /// The operation is not implemented on this OS (yet).
    #[error("operation not supported on this platform: {0}")]
    Unsupported(&'static str),

    /// The caller passed a value the OS must not act on (e.g. a URL that is
    /// not `http(s)`).
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),

    /// The clipboard could not be read or written.
    #[error("clipboard error: {0}")]
    Clipboard(String),

    /// A semantic root could not be determined (e.g. `HOME` unset).
    #[error("cannot resolve semantic root `{root}`: {reason}")]
    RootUnavailable {
        root: crate::SemanticRoot,
        reason: String,
    },

    /// Carrova's state directory cannot be determined from the environment.
    #[error("cannot determine Carrova state directory: {0}")]
    StateDirUnavailable(String),

    /// A network query or send failed.
    #[error("network error: {0}")]
    Network(String),

    /// The user stopped it at an OS prompt (e.g. dismissed the password
    /// request); the text names the operation.
    #[error("cancelled: {0}")]
    Cancelled(&'static str),

    /// A call into the Android (Java) side failed; `cause` names the error
    /// or exception class, never its message (it may hold names or URLs).
    #[error("Android call {call} failed: {cause}")]
    Android { call: &'static str, cause: String },

    /// A phone feature needs a permission the user has not granted (or
    /// Android's restricted settings hold back); names the kind only.
    #[error("phone permission missing: {0:?}")]
    PermissionDenied(crate::phone::PhonePerm),

    /// I/O failure on a specific path.
    #[error("I/O error on {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

impl PlatformError {
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
