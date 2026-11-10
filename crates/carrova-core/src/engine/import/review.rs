//! The one resume policy for the plan under review
//! ([`Importer::plan_for_review`]).

use serde::{Deserialize, Serialize};

use super::Importer;
use crate::error::Result;
use crate::plan::{Plan, PlanOptions};
use crate::session::State;

/// Whether the plan under review still needs the user's approval
/// ([`Importer::plan_for_review`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PlanApproval {
    /// The user must approve it ([`Importer::approve`]).
    Needed,
    /// The user approved this plan, with these options, earlier: apply it
    /// without asking again.
    Given,
}

impl Importer {
    /// The options of the session's current plan (for a resumed session,
    /// the ones the user reviewed and possibly approved); `None` before
    /// the first plan. A front-end resuming a session starts from these
    /// and applies only the choices the user changes, so an unchanged
    /// resume keeps the approval ([`Self::plan_for_review`]).
    #[must_use]
    pub fn plan_options(&self) -> Option<&PlanOptions> {
        self.plan.as_ref().map(|p| &p.options)
    }

    /// The plan to review for `options` — the one resume policy every
    /// front-end shares. A session whose current plan the user already
    /// approved with these same options (a resumed session) keeps that
    /// plan and its approval ([`PlanApproval::Given`]: apply it without
    /// asking again); anything else is planned afresh ([`Self::plan`])
    /// and needs [`Self::approve`] ([`PlanApproval::Needed`]).
    pub fn plan_for_review(&mut self, options: &PlanOptions) -> Result<(&Plan, PlanApproval)> {
        let approved_as_asked = self.s.record.state == State::Planned
            && self.s.record.approved
            && self.plan.as_ref().is_some_and(|p| p.options == *options);
        if approved_as_asked && let Some(plan) = self.plan.take() {
            return Ok((self.plan.insert(plan), PlanApproval::Given));
        }
        Ok((self.plan(options)?, PlanApproval::Needed))
    }
}
