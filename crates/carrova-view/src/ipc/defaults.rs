//! The engine's suggestions a front-end starts from, as one value: the
//! desktop gets it as the generated `ENGINE_DEFAULTS` constant
//! (`bindings/constants.ts`), so it never keeps its own copy.

use carrova_core::PlanOptions;
use serde::Serialize;

use crate::passphrase::PassphrasePolicy;

/// Suggestions and policies front-ends start from. Every suggestion stays
/// the user's to change.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct EngineDefaults {
    /// The suggested import options ([`PlanOptions::default`]): every
    /// offered item the receiver does not treat as sensitive, back up and
    /// replace on conflicts, nothing overridden.
    pub plan_options: PlanOptions,
    /// How strong an archive passphrase must be.
    pub passphrase: PassphrasePolicy,
}

impl EngineDefaults {
    /// This build's defaults.
    #[must_use]
    pub fn current() -> Self {
        Self {
            plan_options: PlanOptions::default(),
            passphrase: PassphrasePolicy::current(),
        }
    }
}
