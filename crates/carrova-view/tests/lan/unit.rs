use carrova_core::lan::{InventoryApp, ItemPlanSummary};
use carrova_platform::SemanticRoot;

use super::*;

fn ticket(s: &str) -> Ticket {
    s.parse().unwrap()
}

#[test]
fn code_view_offers_fallback_ticket_only_with_an_address() {
    let v = code_view(
        &ticket("7-acid-lemon@192.168.1.20:47822"),
        CodeOffer::First,
        Duration::from_secs(300),
    );
    assert_eq!(v.code, "7-acid-lemon");
    assert_eq!(
        v.fallback.as_deref(),
        Some("7-acid-lemon@192.168.1.20:47822")
    );
    assert!(!v.renewed);
    assert_eq!(v.expires_in_secs, 300);
    let v = code_view(&ticket("7-acid-lemon"), CodeOffer::Renewed, Duration::ZERO);
    assert_eq!(v.fallback, None);
    assert!(v.renewed);
}

#[test]
fn inventory_view_sorts_apps() {
    let inv = Inventory {
        os: OsKind::Linux,
        carrova_version: "0.1.0".into(),
        apps: ["zsh", "git"]
            .map(|id| InventoryApp {
                id: id.into(),
                version: None,
            })
            .into(),
        free_space: Some(5),
        device_name: None,
    };
    let v = inventory_view(&inv);
    assert_eq!(v.os, OsKind::Linux);
    assert_eq!(v.apps, ["git", "zsh"]);
    assert_eq!(v.free_space, Some(5));
}

fn summary(conflicts: usize) -> PlanSummary {
    PlanSummary {
        items: vec![ItemPlanSummary {
            item: "git/config".into(),
            counts: ActionCounts {
                create: 1,
                replace: 2,
                ..ActionCounts::default()
            },
            conflicts: (0..conflicts)
                .map(|i| ConflictTarget {
                    root: SemanticRoot::Home,
                    rel: format!("f{i}"),
                })
                .collect(),
        }],
    }
}

#[test]
fn summary_view_caps_conflicts_unless_full() {
    let s = summary(CONFLICTS_SHOWN + 3);
    let v = summary_view(&s, Detail::Summary);
    assert_eq!(v.conflicts, CONFLICTS_SHOWN + 3);
    assert!(v.truncated);
    assert_eq!(v.items[0].conflicts.len(), CONFLICTS_SHOWN);
    assert_eq!(v.items[0].hidden_conflicts, 3);
    assert_eq!(v.items[0].counts.replace, 2);
    let v = summary_view(&s, Detail::Full);
    assert!(!v.truncated);
    assert_eq!(v.items[0].conflicts.len(), CONFLICTS_SHOWN + 3);
}

#[test]
fn pause_view_reports_shortfall() {
    let id = SessionId("20261108-101500-a1b2c3".into());
    let reason = PauseReason::LowDiskSpace {
        fs: "/home/me/.local/state/carrova".into(),
        available: 100,
        needed: 250,
        disk: Some("/home".into()),
    };
    let v = pause_view(&reason, Some(&id)).unwrap();
    assert_eq!(v.disk, Some("/home"));
    assert_eq!((v.shortfall, v.available), (150, 100));
    assert_eq!(v.fs, "/home/me/.local/state/carrova");
    assert_eq!(v.session, Some(&id));
}
