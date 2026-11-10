//! Presentation model of LAN sessions: what the sender shows about the
//! receiver (inventory, plan summary), the pairing-code display and the
//! receiver's pause notice. Pure (no I/O, no escaping):
//! front-end renderers escape every peer-provided string (the CLI uses
//! `render::text::safe`).

use carrova_core::lan::{ActionCounts, ConflictTarget, Inventory, PlanSummary};
use carrova_core::session::{PauseReason, SessionId};
use carrova_core::{CodeOffer, ItemKey, Ticket};
use carrova_platform::OsKind;
use serde::Serialize;

use super::Detail;

/// Conflicts listed per item in [`Detail::Summary`]; the rest are counted.
pub const CONFLICTS_SHOWN: usize = 5;

/// The pairing code as shown once to the user, plus the fallback ticket.
///
/// Holds the code: only ever written to the terminal (or, with `--json`,
/// to stdout as an explicit `pairing_code` record), never to events or
/// logs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CodeView {
    /// The code the receiver types (`7-acid-lemon`).
    pub code: String,
    /// `<code>@<ipv4>:<port>`, typed on the receiver when discovery cannot
    /// find this machine; `None` when the listener has no fallback address.
    pub fallback: Option<String>,
    /// A previous code was burned by a failed pairing attempt.
    pub renewed: bool,
}

/// The view of `ticket`, offered as `offer`.
pub fn code_view(ticket: &Ticket, offer: CodeOffer) -> CodeView {
    CodeView {
        code: ticket.code.to_string(),
        fallback: ticket.addr.is_some().then(|| ticket.to_string()),
        renewed: offer == CodeOffer::Renewed,
    }
}

/// The receiver's inventory, as shown on the sender.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct InventoryView<'a> {
    /// OS family (rendered with its `Display` name).
    pub os: OsKind,
    /// Receiver's Carrova version (peer-provided).
    pub carrova_version: &'a str,
    /// Recipe ids detected there (peer-provided), sorted.
    pub apps: Vec<&'a str>,
    /// Free space of the receiver's data filesystem, if it knows.
    pub free_space: Option<u64>,
    /// The receiver's device name (sanitised), if it announced one.
    pub device_name: Option<String>,
}

/// View of the receiver's inventory.
pub fn inventory_view(inv: &Inventory) -> InventoryView<'_> {
    let mut apps: Vec<&str> = inv.apps.iter().map(|a| a.id.as_str()).collect();
    apps.sort_unstable();
    InventoryView {
        os: inv.os,
        carrova_version: &inv.carrova_version,
        apps,
        free_space: inv.free_space,
        device_name: inv.device_name(),
    }
}

/// One item of the receiver's plan summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SummaryItemView<'a> {
    /// Item key (peer-provided).
    pub key: &'a ItemKey,
    /// Paths per action.
    pub counts: ActionCounts,
    /// Conflicts listed individually.
    pub conflicts: &'a [ConflictTarget],
    /// Conflicts not listed (only counted).
    pub hidden_conflicts: usize,
}

/// The receiver's plan summary, as shown on the sender.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct SummaryView<'a> {
    /// Items in plan order.
    pub items: Vec<SummaryItemView<'a>>,
    /// Total conflicts.
    pub conflicts: usize,
    /// Some conflicts were not listed.
    pub truncated: bool,
}

/// View of `summary`: [`Detail::Summary`] lists at most
/// [`CONFLICTS_SHOWN`] conflicts per item, [`Detail::Full`] all.
pub fn summary_view(summary: &PlanSummary, detail: Detail) -> SummaryView<'_> {
    let limit = match detail {
        Detail::Summary => CONFLICTS_SHOWN,
        Detail::Full => usize::MAX,
    };
    let items: Vec<SummaryItemView<'_>> = summary
        .items
        .iter()
        .map(|i| {
            let shown = i.conflicts.len().min(limit);
            SummaryItemView {
                key: &i.item,
                counts: i.counts,
                conflicts: &i.conflicts[..shown],
                hidden_conflicts: i.conflicts.len() - shown,
            }
        })
        .collect();
    SummaryView {
        conflicts: summary.items.iter().map(|i| i.conflicts.len()).sum(),
        truncated: items.iter().any(|i| i.hidden_conflicts > 0),
        items,
    }
}

/// Why and how long the receiver waits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PauseView<'a> {
    /// Where space is short (Carrova's state dir; engine-provided path).
    pub fs: &'a str,
    /// Bytes to free before the transfer can continue.
    pub shortfall: u64,
    /// Free bytes now.
    pub available: u64,
    /// The session to resume after Ctrl-C, once known.
    pub session: Option<&'a SessionId>,
}

/// View of a low-disk-space pause; `None` for a pause the user asked
/// for (there is nothing to explain or free up).
pub fn pause_view<'a>(
    reason: &'a PauseReason,
    session: Option<&'a SessionId>,
) -> Option<PauseView<'a>> {
    match reason {
        PauseReason::LowDiskSpace {
            fs,
            available,
            needed,
        } => Some(PauseView {
            fs,
            shortfall: needed.saturating_sub(*available),
            available: *available,
            session,
        }),
        PauseReason::ByUser => None,
    }
}

#[cfg(test)]
#[path = "../tests/lan/unit.rs"]
mod tests;
