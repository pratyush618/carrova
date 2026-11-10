//! The receiver's review step, shared by every front-end: the plan to
//! show for the user's options (under the one resume policy,
//! [`Importer::plan_for_review`]) and the space it needs on each
//! filesystem.

use carrova_core::plan::{Plan, SpaceNeed, space_needs};
use carrova_core::{Importer, PlanApproval, PlanOptions};
use carrova_platform::Platform;

use crate::ipc::PlanReview;

/// A plan ready for review.
#[derive(Debug, Clone)]
pub struct Prepared {
    /// The plan to show.
    pub plan: Plan,
    /// Free space it needs per filesystem (empty when the source's
    /// manifest can no longer be read).
    pub needs: Vec<SpaceNeed>,
    /// Whether the user still has to approve it.
    pub approval: PlanApproval,
}

/// The plan of `importer` for `options` and the space it needs. A
/// resumed session's approved plan is reused when the options are
/// unchanged; anything else is planned afresh and needs approval.
///
/// # Errors
///
/// The session cannot plan (wrong state, unknown items in `options`), or
/// the plan does not match the manifest.
pub fn prepare(
    importer: &mut Importer,
    platform: &dyn Platform,
    options: &PlanOptions,
) -> carrova_core::Result<Prepared> {
    let (plan, approval) = importer.plan_for_review(options)?;
    let plan = plan.clone();
    let needs = match importer.manifest() {
        Some(manifest) => space_needs(platform, &plan, manifest)?,
        None => Vec::new(),
    };
    Ok(Prepared {
        plan,
        needs,
        approval,
    })
}

impl Prepared {
    /// Everything the review screens show, with the source's `manifest`
    /// ([`Importer::manifest`]) for the offered items.
    #[must_use]
    pub fn review<'a>(&'a self, manifest: Option<&'a carrova_core::Manifest>) -> PlanReview<'a> {
        PlanReview::new(&self.plan, manifest, &self.needs, self.approval)
    }
}
