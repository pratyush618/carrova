//! Error types.

use std::path::PathBuf;

use carrova_crypto::sign::SignError;
use carrova_platform::PlatformError;
use carrova_recipe::LoadError;

use crate::{BlobHash, BundleVersion};

/// Errors from building, verifying, fetching or installing bundles.
#[derive(Debug, thiserror::Error)]
pub enum BundleError {
    /// The recipe tree has load errors or validation issues (build refused).
    #[error("recipe tree is invalid:\n{}", .0.join("\n"))]
    InvalidRecipes(Vec<String>),
    /// `TIERS.yaml` is malformed or names an unknown recipe.
    #[error("invalid tier policy: {0}")]
    Tiers(String),
    /// A local file could not be read or written.
    #[error("I/O error on {path}: {source}")]
    Io {
        /// File involved.
        path: PathBuf,
        /// Underlying error.
        #[source]
        source: std::io::Error,
    },
    /// A platform file operation failed.
    #[error(transparent)]
    Platform(#[from] PlatformError),
    /// A JSON document could not be parsed or written.
    #[error("invalid {what}: {source}")]
    Json {
        /// Which document.
        what: &'static str,
        /// Underlying error.
        #[source]
        source: serde_json::Error,
    },
    /// Signature or key-chain verification failed.
    #[error("bundle signature rejected: {0}")]
    Signature(#[from] SignError),
    /// The index declares a format this client does not understand.
    #[error("unsupported bundle format {found} (supported: {supported})")]
    UnsupportedFormat {
        /// Declared format.
        found: u32,
        /// Supported format.
        supported: u32,
    },
    /// The index is structurally invalid (duplicate id, oversize entry…).
    #[error("invalid bundle index: {0}")]
    InvalidIndex(String),
    /// The offered bundle is older than the installed one.
    #[error("bundle version {offered} is older than installed version {installed}")]
    Rollback {
        /// Version offered.
        offered: BundleVersion,
        /// Version installed.
        installed: BundleVersion,
    },
    /// The offered version is further ahead of the installed one than
    /// [`crate::MAX_VERSION_JUMP`] allows.
    #[error(
        "bundle version {offered} jumps more than {max} versions past installed version {installed}"
    )]
    VersionJump {
        /// Version offered.
        offered: BundleVersion,
        /// Version installed.
        installed: BundleVersion,
        /// [`crate::MAX_VERSION_JUMP`].
        max: u64,
    },
    /// The offered bundle is newer but was created before the installed one.
    #[error("bundle was created at {offered}, before the installed bundle ({installed})")]
    CreatedBackwards {
        /// `created` of the offered bundle (unix seconds).
        offered: u64,
        /// `created` of the installed bundle (unix seconds).
        installed: u64,
    },
    /// The user rolled this version back; it is never installed again.
    #[error("bundle version {0} was rolled back and is not installed again")]
    Refused(BundleVersion),
    /// The installed bundle's signature chain no longer verifies against
    /// the trusted roots (e.g. its root was removed: revoked).
    #[error("installed recipe bundle {version} is no longer trusted: {source}")]
    InstalledUntrusted {
        /// Installed version.
        version: BundleVersion,
        /// Why the chain was rejected.
        #[source]
        source: SignError,
    },
    /// The shipped bundle's `index.json` is not the one pinned at build
    /// time.
    #[error("shipped recipe bundle index {found} does not match the pinned {expected}")]
    ShippedPinMismatch {
        /// Hash pinned into the binary.
        expected: BlobHash,
        /// Hash of the shipped index.
        found: BlobHash,
    },
    /// The release pointer (`recipes-latest/latest.json`) is malformed.
    #[error("invalid release pointer: {0}")]
    InvalidPointer(String),
    /// The release the pointer names holds a different bundle version.
    #[error("release pointer names {pointer} but the release holds {index}")]
    PointerMismatch {
        /// Version named by the pointer.
        pointer: BundleVersion,
        /// Version in the release's index.
        index: BundleVersion,
    },
    /// A hash string is not 64 lowercase hex digits.
    #[error("malformed blob hash `{0}`")]
    MalformedHash(String),
    /// A blob's bytes do not match the hash or size the index records.
    #[error("blob {name} does not match the signed index")]
    BlobMismatch {
        /// Asset file name.
        name: String,
    },
    /// A recipe blob disagrees with its index entry (id, category, schema).
    #[error("recipe blob {name} does not match its index entry: {reason}")]
    EntryMismatch {
        /// Asset file name.
        name: String,
        /// What differs.
        reason: String,
    },
    /// Fetching an asset failed.
    #[error("cannot fetch {name}: {source}")]
    Fetch {
        /// Asset file name.
        name: String,
        /// Underlying error.
        #[source]
        source: FetchError,
    },
    /// A recipe blob does not parse.
    #[error(transparent)]
    Recipe(#[from] LoadError),
    /// No bundle is installed.
    #[error("no recipe bundle is installed")]
    NotInstalled,
    /// Rollback requested but no previous bundle is kept.
    #[error("no previous recipe bundle to roll back to")]
    NoPrevious,
    /// A blob the index needs is not in the local store.
    #[error("blob {name} is missing from the local store")]
    MissingBlob {
        /// Asset file name.
        name: String,
    },
    /// The build output directory is not empty.
    #[error("output directory {0} is not empty")]
    OutputNotEmpty(PathBuf),
    /// The signer key does not match the certificate.
    #[error("signer key does not match the certificate")]
    SignerMismatch,
    /// No recipe source is available.
    #[error("no recipe source available (no dev dir, installed or shipped bundle)")]
    NoRecipes,
    /// The system clock is before 1970, so signatures cannot be checked.
    #[error("the system clock is before 1970; cannot check signatures")]
    Clock,
}

/// Why a [`crate::Fetcher`] could not deliver an asset.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    /// The asset does not exist.
    #[error("not found")]
    NotFound,
    /// The asset is larger than allowed.
    #[error("larger than {limit} bytes")]
    TooLarge {
        /// Byte limit.
        limit: u64,
    },
    /// The configuration is unusable (e.g. a plain-HTTP URL when HTTPS is
    /// required).
    #[error("invalid configuration: {0}")]
    Config(String),
    /// HTTP or network failure.
    #[error("transport error: {0}")]
    Transport(String),
    /// Local read failure.
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Crate result alias.
pub type Result<T, E = BundleError> = std::result::Result<T, E>;

impl BundleError {
    /// [`BundleError::Io`] for `path`.
    pub(crate) fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    /// [`BundleError::Json`] for `what`.
    pub(crate) fn json(what: &'static str) -> impl FnOnce(serde_json::Error) -> Self {
        move |source| Self::Json { what, source }
    }
}
