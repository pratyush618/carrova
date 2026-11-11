//! Owned command replies. The view-models borrow from engine objects that
//! live on a session's worker thread; a reply owns the engine data its
//! view needs, so it can leave that thread, and serialises exactly as the
//! view (the TypeScript type named on each).

use carrova_core::{Manifest, PickOutcome, ScanReport, Selection, SessionId};
use serde::{Serialize, Serializer};

use super::{Picked, PlanReview, ScanResult};
use crate::review::Prepared;

/// Reply of `scan`, `suggest` and `select`: serialises as [`ScanResult`].
#[derive(Debug, Clone)]
pub struct ScanReply {
    session: SessionId,
    report: ScanReport,
    selection: Selection,
}

impl ScanReply {
    /// `selection` of export session `session`, against its scan `report`.
    #[must_use]
    pub fn new(session: SessionId, report: ScanReport, selection: Selection) -> Self {
        Self {
            session,
            report,
            selection,
        }
    }

    /// The reply as its view.
    #[must_use]
    pub fn result(&self) -> ScanResult<'_> {
        ScanResult::new(self.session.clone(), &self.report, &self.selection)
    }
}

impl Serialize for ScanReply {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.result().serialize(serializer)
    }
}

/// Reply of `pick`: serialises as [`Picked`].
#[derive(Debug, Clone)]
pub struct PickedReply {
    scan: ScanReply,
    outcomes: Vec<PickOutcome>,
}

impl PickedReply {
    /// The selection `scan` after the picks, with one outcome per path.
    #[must_use]
    pub fn new(scan: ScanReply, outcomes: Vec<PickOutcome>) -> Self {
        Self { scan, outcomes }
    }

    /// The reply as its view.
    #[must_use]
    pub fn picked(&self) -> Picked<'_> {
        Picked {
            scan: self.scan.result(),
            outcomes: &self.outcomes,
        }
    }
}

impl Serialize for PickedReply {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.picked().serialize(serializer)
    }
}

/// Reply of `plan`: serialises as [`PlanReview`].
#[derive(Debug, Clone)]
pub struct PlanReply {
    prepared: Prepared,
    manifest: Option<Manifest>,
}

impl PlanReply {
    /// The review of `prepared`, with the source's `manifest`
    /// ([`carrova_core::Importer::manifest`]) for the offered items.
    #[must_use]
    pub fn new(prepared: Prepared, manifest: Option<Manifest>) -> Self {
        Self { prepared, manifest }
    }

    /// The reply as its view.
    #[must_use]
    pub fn review(&self) -> PlanReview<'_> {
        self.prepared.review(self.manifest.as_ref())
    }
}

impl Serialize for PlanReply {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.review().serialize(serializer)
    }
}
