//! The error every IPC command returns, and the one place any failure
//! becomes a [`BridgeError`] (`{ code, message, hint }`).
//!
//! Messages are the engine's own texts (the CLI shows the same ones; they
//! never hold secrets or file contents), with the cause chain appended.
//! Pairing codes and passphrases never reach an error: ticket parse
//! failures get a fixed text. Hints speak in the app's terms ("resume it
//! from History"), not CLI flags.

use std::error::Error;
use std::fmt;

use carrova_bundle::{BundleError, FetchError};
use carrova_core::{CoreError, ErrorClass, Hint};
use carrova_view::ipc::{BridgeError, ErrorCode};
use carrova_view::passphrase::PassphraseRejected;
use serde::Serialize;

/// A failed command, serialised exactly as its [`BridgeError`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct CommandError {
    bridge: BridgeError,
    /// The engine failed (as opposed to the bridge refusing the request):
    /// such failures may become error reports.
    #[serde(skip)]
    engine: bool,
}

impl CommandError {
    /// An error of class `code` with `message` and no hint.
    pub(crate) fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            bridge: BridgeError {
                code,
                message: message.into(),
                hint: None,
            },
            engine: false,
        }
    }

    /// The same error with a recovery hint.
    #[must_use]
    pub(crate) fn with_hint(mut self, hint: impl Into<String>) -> Self {
        self.bridge.hint = Some(hint.into());
        self
    }

    /// "No such live session" (or not one that supports the command).
    pub(crate) fn no_session(what: &str) -> Self {
        Self::new(ErrorCode::Session, what)
    }

    /// A session worker stopped (it panicked); the session is unusable.
    pub(crate) fn worker_gone() -> Self {
        Self::new(ErrorCode::Internal, "the session stopped unexpectedly")
            .with_hint("Resume it from History, or start again.")
    }

    /// A failure while pairing with a sender (`connect`), classed by
    /// [`CoreError::class_when_connecting`].
    pub(crate) fn when_connecting(error: &CoreError) -> Self {
        Self::engine(error.class_when_connecting(), error)
    }

    /// The error as the UI receives it.
    #[must_use]
    pub fn bridge(&self) -> &BridgeError {
        &self.bridge
    }

    /// Whether the engine failed (worth an error report, consent given),
    /// rather than the bridge refusing the request.
    #[must_use]
    pub fn is_engine_failure(&self) -> bool {
        self.engine
    }

    /// `error` with its cause chain as the message.
    fn chained(code: ErrorCode, error: &(dyn Error + 'static)) -> Self {
        Self::new(code, message_chain(error))
    }

    /// Engine `error` of class `class`, its hint in the app's words.
    fn engine(class: ErrorClass, error: &CoreError) -> Self {
        let mut mapped = Self::chained(class.kind.into(), error);
        mapped.bridge.hint = class.hint.map(|hint| hint_text(hint).to_owned());
        mapped.engine = true;
        mapped
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.bridge.message)
    }
}

impl Error for CommandError {}

impl From<CommandError> for BridgeError {
    fn from(error: CommandError) -> Self {
        error.bridge
    }
}

/// `error` and every `source()` below it, joined with `": "`.
fn message_chain(error: &(dyn Error + 'static)) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        // `#[error(transparent)]` wrappers repeat their cause verbatim.
        if !message.ends_with(&text) {
            message.push_str(": ");
            message.push_str(&text);
        }
        source = cause.source();
    }
    message
}

/// The engine's recovery advice in the app's terms (screens, not CLI
/// flags): the one wording of every [`Hint`].
pub(crate) const fn hint_text(hint: Hint) -> &'static str {
    match hint {
        Hint::ResumeWhenBack => {
            "The session is kept: resume it from History when both machines are ready."
        }
        Hint::ResumeAfterPeerCancel => {
            "The other machine cancelled. The session is kept: resume it from History when \
             both machines are ready, or dismiss it."
        }
        Hint::PeerEnded => "The other machine ended the session. Nothing was applied.",
        Hint::DeselectUnsupplied => {
            "Nothing was applied. Resume the session and deselect these items, or retry once \
             the files are readable on the other machine."
        }
        Hint::CheckCode => {
            "Check the code shown on the other machine now (a wrong attempt replaces it), or \
             type the code@address line shown there."
        }
        Hint::ResolveBlockers => {
            "Resolve the blockers, deselect those items, or override where offered."
        }
        Hint::NothingChanged => "Nothing was changed: the apply was rolled back.",
        Hint::ManualAttention => "Some changes need manual attention: see the session in History.",
        Hint::CheckPassphrase => "Check the passphrase; if it is right, the file is damaged.",
    }
}

impl From<CoreError> for CommandError {
    fn from(error: CoreError) -> Self {
        Self::engine(error.class(), &error)
    }
}

impl From<BundleError> for CommandError {
    fn from(error: BundleError) -> Self {
        match &error {
            BundleError::NoRecipes => Self::new(
                ErrorCode::Recipes,
                "no recipes available: no verified recipe bundle is installed",
            )
            .with_hint("Update recipes in Settings to download the verified bundle."),
            BundleError::Fetch {
                source: FetchError::Transport(_),
                ..
            } => Self::chained(ErrorCode::Network, &error)
                .with_hint("Check your network connection and try again."),
            BundleError::Signature(_) => Self::chained(ErrorCode::Recipes, &error)
                .with_hint("Nothing was installed. Try again later; report it if it persists."),
            _ => Self::chained(ErrorCode::Recipes, &error),
        }
    }
}

impl From<PassphraseRejected> for CommandError {
    fn from(error: PassphraseRejected) -> Self {
        Self::new(ErrorCode::WrongPassphrase, error.to_string())
    }
}

/// Errors that only ever mean "something went wrong inside the app".
macro_rules! internal_errors {
    ($($error:ty),* $(,)?) => {$(
        impl From<$error> for CommandError {
            fn from(error: $error) -> Self {
                Self::chained(ErrorCode::Internal, &error)
            }
        }
    )*};
}

internal_errors!(
    carrova_report::ReportError,
    carrova_platform::PlatformError,
    serde_json::Error,
    std::io::Error,
    tauri::Error,
);

impl From<tauri_plugin_updater::Error> for CommandError {
    fn from(error: tauri_plugin_updater::Error) -> Self {
        Self::chained(ErrorCode::Network, &error)
            .with_hint("Check your network connection and try again.")
    }
}

#[cfg(test)]
#[path = "../tests/command_error/unit.rs"]
mod tests;
