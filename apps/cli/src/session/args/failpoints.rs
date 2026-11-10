//! Test-only flags: hidden, and inert unless built with feature
//! `failpoints` (then `--fail-after N` fails apply after N actions).

use clap::Args;

/// Test-only flags (empty without feature `failpoints`).
#[derive(Debug, Args)]
pub struct FailpointArgs {
    /// Fail apply after N actions (fault injection, tests only).
    #[cfg(feature = "failpoints")]
    #[arg(long, hide = true, value_name = "N")]
    fail_after: Option<usize>,
}

#[cfg(feature = "failpoints")]
impl FailpointArgs {
    /// Make `importer`'s apply fail where `--fail-after` says, if given.
    pub fn arm(&self, importer: &mut carrova_core::Importer) {
        if let Some(actions) = self.fail_after {
            importer.set_failpoint(carrova_core::Failpoint::after(actions));
        }
    }
}
