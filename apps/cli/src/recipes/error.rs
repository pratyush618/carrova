//! Recipe bundle errors explained for the terminal: the one place a
//! [`BundleError`] becomes a user-facing message.

use anyhow::anyhow;
use carrova_bundle::{BundleError, FetchError};

/// What the user asked for when the error happened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action<'a> {
    /// Loading recipes for a command.
    Load,
    /// `carrova recipes update` from this base URL.
    Update(&'a str),
    /// `carrova recipes rollback`.
    Rollback,
    /// `carrova recipes reset`.
    Reset,
}

/// `err` as a message that says what happened and what to do next. Errors
/// without a better explanation keep their own text.
pub fn explain(err: BundleError, action: Action<'_>) -> anyhow::Error {
    match (&err, action) {
        (BundleError::NoRecipes, _) => anyhow!(
            "no recipes available: no verified recipe bundle is installed. \
             Run `carrova recipes update` to download it \
             (or pass --recipes DIR to use a local recipe tree)"
        ),
        (BundleError::NotInstalled, Action::Rollback) => {
            anyhow!("no recipe bundle is installed; nothing to roll back")
        }
        (BundleError::NoPrevious, Action::Rollback) => anyhow!(
            "no previous recipe bundle is kept (only the bundle replaced by the \
             last update can be restored)"
        ),
        (
            BundleError::Fetch {
                source: FetchError::Config(_),
                ..
            },
            Action::Update(_),
        ) => anyhow::Error::new(err).context("invalid recipe bundle URL"),
        (
            BundleError::Fetch {
                source: FetchError::Transport(_),
                ..
            },
            Action::Update(url),
        ) => anyhow::Error::new(err).context(format!(
            "cannot download recipes from {url}; check your network connection \
             and try again"
        )),
        (BundleError::Signature(_), Action::Update(_)) => anyhow::Error::new(err).context(
            "the downloaded recipe bundle is not signed by a trusted key; nothing was installed",
        ),
        (
            BundleError::Rollback {
                offered, installed, ..
            },
            Action::Update(_),
        ) => anyhow!(
            "the server offers recipe bundle {offered}, older than the installed \
             {installed}; refusing to downgrade. Try again later and report it if \
             it persists"
        ),
        _ => anyhow::Error::new(err),
    }
}

/// `carrova recipes update` found no bundle at `url` (HTTP 404, see
/// [`carrova_bundle::UpdateAttempt::RetryLater`]): nothing changed.
pub fn retry_later(url: &str) -> anyhow::Error {
    anyhow!(
        "no recipe bundle found at {url} (HTTP 404). A new bundle may be \
         being published right now: try again in a minute"
    )
}

#[cfg(test)]
#[path = "../../tests/recipes/error/unit.rs"]
mod tests;
