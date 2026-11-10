use carrova_core::{RollbackFailure, RollbackPreview};

use super::*;

#[test]
fn the_preview_counts_every_outcome_and_lists_what_needs_attention() {
    let p = RollbackPreview {
        restored: 2,
        removed: 3,
        moved_aside: 1,
        blockers: vec![RollbackFailure {
            target: "/home/u/.config/app/x\u{1b}".into(),
            reason: "changed since apply; left in place".into(),
        }],
    };
    let mut out = Vec::new();
    rollback_preview(&mut out, &p).unwrap();
    let text = String::from_utf8(out).unwrap();
    assert!(text.starts_with(
        "Undo would do: 2 restored \u{b7} 3 removed \u{b7} 1 moved aside \u{b7} 1 need attention"
    ));
    assert!(
        text.contains("needs attention: /home/u/.config/app/x\\u{1b}"),
        "{text}"
    );
}
