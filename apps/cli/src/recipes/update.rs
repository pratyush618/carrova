//! `carrova recipes update | rollback | reset`: change the installed
//! bundle. Only these commands ever do; nothing updates recipes
//! automatically.

use anyhow::Result;
use carrova_bundle::{BundleError, BundleStore, BundleVersion, UpdateAttempt, UpdateOutcome};

use super::error::{Action, explain, retry_later};
use super::seams::{http_config, trusted_roots};
use crate::prompt::{confirm, ensure_can_confirm};

/// The update attempt for release base `url` as the command's result
/// (errors explained): a bundle that is not published yet is an error
/// that says to retry, since nothing was updated.
pub fn outcome(attempt: carrova_bundle::Result<UpdateAttempt>, url: &str) -> Result<UpdateOutcome> {
    match attempt.map_err(|e| explain(e, Action::Update(url)))? {
        UpdateAttempt::Done(outcome) => Ok(outcome),
        UpdateAttempt::RetryLater => Err(retry_later(url)),
    }
}

/// [`carrova_bundle::update`] over HTTPS with the release roots, into the
/// user's store: `url` is the release download base, whose
/// `recipes-latest/latest.json` names the release to install.
pub fn update(url: &str) -> Result<UpdateOutcome> {
    let roots = trusted_roots().map_err(|e| explain(e, Action::Update(url)))?;
    let platform = carrova_platform::current();
    outcome(
        carrova_bundle::update(platform.as_ref(), roots, &http_config(url)),
        url,
    )
}

/// What `carrova recipes rollback` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollbackResult {
    /// The installed bundle went from `from` back to `to`.
    RolledBack {
        /// Version that was installed.
        from: BundleVersion,
        /// Version now installed.
        to: BundleVersion,
    },
    /// The user said no; nothing changed.
    Declined,
}

/// Make the previous bundle current again after confirmation (`yes`
/// answers it; without a terminal, `yes` is required).
pub fn rollback(yes: bool) -> Result<RollbackResult> {
    ensure_can_confirm(yes, "recipe rollback")?;
    let platform = carrova_platform::current();
    let why = |e| explain(e, Action::Rollback);
    let store = BundleStore::open(platform.as_ref(), trusted_roots().map_err(why)?).map_err(why)?;
    let installed = store
        .installed()
        .map_err(why)?
        .ok_or_else(|| why(BundleError::NotInstalled))?;
    let from = installed.version();
    let to = installed
        .previous
        .ok_or_else(|| why(BundleError::NoPrevious))?;
    if !confirm(yes, &format!("Roll back recipes from {from} to {to}?"))? {
        return Ok(RollbackResult::Declined);
    }
    let to = store.rollback().map_err(why)?;
    Ok(RollbackResult::RolledBack { from, to })
}

/// What `carrova recipes reset` did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResetResult {
    /// The store was reset; `removed` is the bundle that was installed.
    Reset {
        /// Version removed, if one was installed.
        removed: Option<BundleVersion>,
    },
    /// The user said no; nothing changed.
    Declined,
}

/// Remove the installed bundle after confirmation (`yes` answers it;
/// without a terminal, `yes` is required), so the shipped recipes are used
/// again.
pub fn reset(yes: bool) -> Result<ResetResult> {
    ensure_can_confirm(yes, "recipe reset")?;
    let platform = carrova_platform::current();
    let why = |e| explain(e, Action::Reset);
    let store = BundleStore::open(platform.as_ref(), trusted_roots().map_err(why)?).map_err(why)?;
    reset_store(&store, |question| confirm(yes, question))
}

/// Reset `store` if `confirm` agrees.
pub fn reset_store(
    store: &BundleStore<'_>,
    confirm: impl FnOnce(&str) -> Result<bool>,
) -> Result<ResetResult> {
    if !confirm("Remove the installed recipe bundle and use the recipes shipped with Carrova?")? {
        return Ok(ResetResult::Declined);
    }
    let removed = store.reset().map_err(|e| explain(e, Action::Reset))?;
    Ok(ResetResult::Reset { removed })
}
