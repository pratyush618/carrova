use std::path::PathBuf;

/// Errors returned by [`crate::Platform`] implementations.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    /// The operation is not implemented on this OS (yet).
    #[error("operation not supported on this platform: {0}")]
    Unsupported(&'static str),

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
