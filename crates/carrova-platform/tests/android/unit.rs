use super::*;

fn kind(mimes: &[&str]) -> Option<ClipKind> {
    clip_kind(mimes.iter().copied())
}

#[test]
fn text_mimes_read_as_text() {
    assert_eq!(kind(&["text/plain"]), Some(ClipKind::Text));
    assert_eq!(kind(&["text/html", "text/plain"]), Some(ClipKind::Text));
    assert_eq!(kind(&["text/uri-list"]), Some(ClipKind::Text));
}

#[test]
fn png_reads_as_png() {
    assert_eq!(kind(&["image/png"]), Some(ClipKind::Png));
}

#[test]
fn text_wins_over_png() {
    assert_eq!(kind(&["image/png", "text/plain"]), Some(ClipKind::Text));
}

#[test]
fn other_mimes_are_not_synced() {
    assert_eq!(kind(&["image/jpeg"]), None);
    assert_eq!(kind(&["application/pdf"]), None);
    assert_eq!(kind(&[]), None);
}

#[test]
fn absent_sensitive_extra_is_not_sensitive() {
    assert!(!is_sensitive(None));
}

#[test]
fn sensitive_extra_true_is_sensitive() {
    assert!(is_sensitive(Some("true")));
}

#[test]
fn sensitive_extra_false_is_not_sensitive() {
    assert!(!is_sensitive(Some("false")));
}

#[test]
fn unreadable_sensitive_extra_fails_closed() {
    assert!(is_sensitive(Some("")));
    assert!(is_sensitive(Some("1")));
    assert!(is_sensitive(Some("yes")));
}
