//! [`PluginError`]: why a plugin could not be loaded or run.

/// Why a plugin was refused or failed. Messages never carry file contents.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PluginError {
    /// `plugin.yaml` does not parse or breaks a rule.
    #[error("invalid plugin manifest: {0}")]
    Manifest(String),
    /// The module is not valid WebAssembly (or too large).
    #[error("invalid plugin module: {0}")]
    Module(String),
    /// The module imports something; plugins get no host functions.
    #[error("plugin module imports `{module}.{name}`; plugins may not import anything")]
    Import {
        /// Import module name.
        module: String,
        /// Imported item name.
        name: String,
    },
    /// The module breaks the plugin ABI (missing export, wrong type,
    /// result outside its memory).
    #[error("plugin breaks the ABI: {0}")]
    Abi(String),
    /// A context value cannot be encoded (it holds a newline or `=`).
    #[error("plugin context: {0}")]
    Context(String),
    /// The input file is larger than [`crate::MAX_IO_BYTES`].
    #[error(
        "file is too large for a plugin (at most {} bytes)",
        crate::MAX_IO_BYTES
    )]
    InputTooLarge,
    /// The plugin's output is larger than [`crate::MAX_IO_BYTES`].
    #[error("plugin output is too large (at most {} bytes)", crate::MAX_IO_BYTES)]
    OutputTooLarge,
    /// The plugin used up its fuel (manifest `limits.fuel`).
    #[error("plugin ran out of fuel")]
    OutOfFuel,
    /// The plugin used up its fuel for the whole restore
    /// ([`crate::MAX_RUN_FUEL`]).
    #[error("plugin used up its time budget for this restore")]
    BudgetExhausted,
    /// The plugin trapped (including failing to grow memory past its limit).
    #[error("plugin failed: {0}")]
    Trap(String),
    /// The plugin returned a negative status: it refused the input.
    #[error("plugin refused the file (status {0})")]
    Refused(i64),
}

/// A wasmi runtime error as a [`PluginError`]: fuel exhaustion is its own
/// case, everything else is a trap.
pub(crate) fn runtime(err: &wasmi::Error) -> PluginError {
    if err.as_trap_code() == Some(wasmi::TrapCode::OutOfFuel) {
        PluginError::OutOfFuel
    } else {
        PluginError::Trap(err.to_string())
    }
}
