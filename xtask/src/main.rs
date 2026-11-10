//! `cargo xtask <cmd>`: developer tooling that must never ship in the
//! end-user binaries (recipe bundle keys and signing).

use clap::Parser;

mod bundle;

/// Carrova developer tasks.
#[derive(Debug, Parser)]
#[command(name = "xtask")]
enum Cmd {
    /// Recipe bundle tooling: keys, certificates, build, sign, verify.
    #[command(subcommand)]
    Bundle(bundle::BundleCmd),
}

fn main() -> anyhow::Result<()> {
    match Cmd::parse() {
        Cmd::Bundle(cmd) => cmd.run(),
    }
}
