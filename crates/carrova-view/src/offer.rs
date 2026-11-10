//! Presentation model of what the source (archive or sender) offers and
//! which of it the plan options pick. Pure (no I/O, no escaping).

use carrova_core::plan::Plan;
use carrova_core::{ItemKey, Manifest};
use serde::Serialize;

/// Sensitivity of an offered item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct Sensitivity<'a> {
    /// Why it is sensitive (the receiver's judgement, else the source's).
    pub reason: &'a str,
    /// The user confirmed importing it.
    pub confirmed: bool,
}

/// One item the source offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OfferedItem<'a> {
    /// Item key.
    pub key: ItemKey,
    /// Label from the source.
    pub label: &'a str,
    /// Selected for import by the plan options.
    pub picked: bool,
    /// Set when either side considers the item sensitive.
    pub sensitive: Option<Sensitivity<'a>>,
}

/// The offered items of `manifest`, in manifest order, as `plan` picks
/// them ([`Plan::picks`]) with their sensitivity ([`Plan::sensitivity`]):
/// both decided by the engine, shown here.
pub fn offered_items<'a>(manifest: &'a Manifest, plan: &'a Plan) -> Vec<OfferedItem<'a>> {
    manifest
        .items
        .iter()
        .map(|item| {
            let key = ItemKey::of(item);
            OfferedItem {
                label: &item.label,
                picked: plan.picks(item),
                sensitive: plan.sensitivity(item).map(|reason| Sensitivity {
                    reason,
                    confirmed: plan.options.is_confirmed(&key),
                }),
                key,
            }
        })
        .collect()
}
