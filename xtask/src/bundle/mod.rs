//! `cargo xtask bundle …`: recipe bundle keys, certificates and releases.
//!
//! Typical release flow (see `keys/README.md`):
//!
//! ```text
//! keygen --role root   --out ~/.config/carrova-dev/bundle-root.key     (offline, once)
//! keygen --role signer --out ~/.config/carrova-dev/bundle-signer.key   (CI secret)
//! certify --root ROOT.key --signer-pub HEX --days 90 --out signer.cert.json
//! build  --recipes recipes --out dist/recipes --version N   (+ ../plugins; `just plugins` first)
//! sign   --dir dist/recipes --signer SIGNER.key --cert signer.cert.json
//! verify --dir dist/recipes --roots keys/bundle-roots.json
//! pin    --dir dist/recipes      (prints CARROVA_SHIPPED_INDEX_HASH for packaging)
//! ```

use std::path::PathBuf;

use clap::Subcommand;

mod keyfile;
mod keys;
mod release;

use keyfile::Role;

/// Bundle subcommands.
#[derive(Debug, Subcommand)]
pub(crate) enum BundleCmd {
    /// Generate a key pair; the secret file is created with mode 0600 and
    /// only the public key is printed.
    Keygen {
        /// What the key is for.
        #[arg(long, value_enum)]
        role: Role,
        /// Secret key file to create (must not exist).
        #[arg(long)]
        out: PathBuf,
    },
    /// Certify a signer public key with a root key.
    Certify {
        /// Root secret key file.
        #[arg(long)]
        root: PathBuf,
        /// Signer public key (hex).
        #[arg(long)]
        signer_pub: String,
        /// Validity in days (at most 90).
        #[arg(long)]
        days: u64,
        /// Certificate file to write.
        #[arg(long)]
        out: PathBuf,
    },
    /// Build an unsigned bundle from a recipe tree.
    Build {
        /// Recipe tree (with TIERS.yaml).
        #[arg(long)]
        recipes: PathBuf,
        /// Output directory (must be empty or absent).
        #[arg(long)]
        out: PathBuf,
        /// Bundle version (strictly greater than the last release).
        #[arg(long)]
        version: u64,
    },
    /// Sign a built bundle.
    Sign {
        /// Bundle directory.
        #[arg(long)]
        dir: PathBuf,
        /// Signer secret key file.
        #[arg(long)]
        signer: PathBuf,
        /// Signer certificate file.
        #[arg(long)]
        cert: PathBuf,
    },
    /// Verify a signed bundle against a roots file, as a client would.
    Verify {
        /// Bundle directory.
        #[arg(long)]
        dir: PathBuf,
        /// Roots file (`{ "roots": [...] }`).
        #[arg(long)]
        roots: PathBuf,
    },
    /// Print the BLAKE3 of a bundle's `index.json`: the value packaging
    /// exports as `CARROVA_SHIPPED_INDEX_HASH` to pin the shipped bundle.
    Pin {
        /// Bundle directory.
        #[arg(long)]
        dir: PathBuf,
    },
}

impl BundleCmd {
    /// Run the subcommand.
    pub(crate) fn run(self) -> anyhow::Result<()> {
        let now = carrova_bundle::now_unix()?;
        match self {
            Self::Keygen { role, out } => keys::keygen(role, &out),
            Self::Certify {
                root,
                signer_pub,
                days,
                out,
            } => keys::certify(&root, &signer_pub, days, &out, now),
            Self::Build {
                recipes,
                out,
                version,
            } => release::build(&recipes, &out, version, now),
            Self::Sign { dir, signer, cert } => release::sign(&dir, &signer, &cert, now),
            Self::Verify { dir, roots } => release::verify(&dir, &roots, now),
            Self::Pin { dir } => release::pin(&dir),
        }
    }
}
