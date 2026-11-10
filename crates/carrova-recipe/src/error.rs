use std::path::PathBuf;

/// Failure to read or parse a recipe file. Always carries the file path.
#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("cannot read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid recipe {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml_ng::Error,
    },
    #[error("{path}: unsupported recipe schema version {found} (supported: {supported})")]
    UnsupportedSchema {
        path: PathBuf,
        found: u64,
        supported: String,
    },
    #[error("{dir}: app directory has no {file}", file = crate::RECIPE_FILE)]
    MissingRecipe { dir: PathBuf },
}

impl LoadError {
    /// File or directory the error refers to.
    #[must_use]
    pub fn path(&self) -> &std::path::Path {
        match self {
            Self::Io { path, .. }
            | Self::Parse { path, .. }
            | Self::UnsupportedSchema { path, .. } => path,
            Self::MissingRecipe { dir } => dir,
        }
    }
}
