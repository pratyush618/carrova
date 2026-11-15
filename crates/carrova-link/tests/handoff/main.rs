//! Public-API tests for handoff URL checks.

use carrova_link::{LinkError, check_url};

#[test]
fn handoff_rejects_non_http() {
    for bad in [
        "file:///etc/passwd",
        "javascript:alert(1)",
        "data:text/html,<script>1</script>",
        "ftp://x.example",
        "not a url",
        "http://",
    ] {
        assert!(matches!(check_url(bad), Err(LinkError::UnsafeUrl)), "{bad}");
    }
    for ok in ["HTTP://ok.example", "https://ok.example/a?b=c"] {
        assert!(check_url(ok).is_ok(), "{ok}");
    }
}

#[test]
fn check_url_rejects_inputs_the_parser_would_silently_rewrite() {
    for bad in [
        " http://x.example",
        "http://a\nb.example",
        "http://a\tb.example",
        "http:/x.example",
        "http:x.example",
        "http:\\\\x.example",
        "http:///x.example",
        "http://evil@good.example",
        "http://user:pw@good.example",
    ] {
        assert!(
            matches!(check_url(bad), Err(LinkError::UnsafeUrl)),
            "{bad:?}"
        );
    }
}

#[test]
fn check_url_returns_the_normalised_url() {
    assert_eq!(
        check_url("HTTP://Ok.Example").unwrap().as_str(),
        "http://ok.example/"
    );
}

#[test]
fn handoff_rejects_urls_over_8_kib() {
    let at_cap = format!("https://a.example/{}", "x".repeat(8192 - 18));
    assert_eq!(at_cap.len(), 8192);
    assert!(check_url(&at_cap).is_ok());
    assert!(matches!(
        check_url(&format!("{at_cap}x")),
        Err(LinkError::TooBig { .. })
    ));
}
