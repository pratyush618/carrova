use super::{clean_mime, host, offer_id};

#[test]
fn host_is_cut_from_a_checked_url() {
    assert_eq!(host("https://example.com/a?b=c"), "example.com");
    assert_eq!(host("http://example.com:8080"), "example.com");
    assert_eq!(host("https://[::1]:443/x"), "::1");
    assert_eq!(host("https://example.com#top"), "example.com");
}

#[test]
fn only_short_type_subtype_mimes_pass() {
    assert_eq!(
        clean_mime(Some("text/plain")).as_deref(),
        Some("text/plain")
    );
    assert_eq!(
        clean_mime(Some("application/vnd.ms-excel")).as_deref(),
        Some("application/vnd.ms-excel")
    );
    assert_eq!(clean_mime(Some("text")), None);
    assert_eq!(clean_mime(Some("text/plain; charset=x")), None);
    assert_eq!(clean_mime(Some("a/b/c")), None);
    assert_eq!(clean_mime(Some(&format!("a/{}", "b".repeat(200)))), None);
    assert_eq!(clean_mime(None), None);
}

#[test]
#[expect(
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the f64 round-trip is the point"
)]
fn offer_ids_survive_a_javascript_number() {
    for _ in 0..1000 {
        let id = offer_id().unwrap();
        assert_eq!(id as f64 as u64, id, "{id} is not exact as a JS number");
    }
}
