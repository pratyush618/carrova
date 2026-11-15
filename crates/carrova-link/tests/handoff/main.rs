//! Public-API tests for handoff URL checks and the `Handoff` lane.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::thread;

use carrova_link::{
    FileOffer, Handoff, LaneKind, LinkError, OfferAnswer, ReceivedHandoff, accept_lane, check_url,
    recv_handoff, send_handoff,
};
use support::{Open, T, linked};

/// Send `handoff`; the receiver answers with `answer`. Returns both sides' results.
fn exchange(
    handoff: &Handoff,
    answer: OfferAnswer,
) -> (
    Result<OfferAnswer, LinkError>,
    Result<Option<ReceivedHandoff>, LinkError>,
) {
    let (a, b) = linked();
    thread::scope(|s| {
        let server = s.spawn(|| {
            let (kind, stream) = accept_lane(&b, &Open, T).unwrap();
            assert_eq!(kind, LaneKind::Handoff);
            recv_handoff(stream, &mut |_| answer)
        });
        (send_handoff(&a, handoff), server.join().unwrap())
    })
}

const ACCEPT: OfferAnswer = OfferAnswer::Accept { resume_from: 0 };

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

#[test]
fn handoff_url_roundtrip() {
    let (sent, got) = exchange(&Handoff::Url("HTTP://Ok.Example".into()), ACCEPT);
    assert_eq!(sent.unwrap(), ACCEPT);
    let Some(ReceivedHandoff::Url(url)) = got.unwrap() else {
        panic!("expected a url")
    };
    assert_eq!(url.as_str(), "http://ok.example/");
}

#[test]
fn declined_handoff_reaches_the_sender_and_yields_nothing() {
    let (sent, got) = exchange(
        &Handoff::Url("https://ok.example".into()),
        OfferAnswer::Decline,
    );
    assert_eq!(sent.unwrap(), OfferAnswer::Decline);
    assert!(got.unwrap().is_none());
}

#[test]
fn receiver_refuses_a_hostile_url_the_sender_never_checked() {
    let (a, b) = linked();
    thread::scope(|s| {
        let server =
            s.spawn(|| recv_handoff(accept_lane(&b, &Open, T).unwrap().1, &mut |_| ACCEPT));
        let mut stream = carrova_link::open_lane(&a, LaneKind::Handoff).unwrap();
        stream
            .send(&Handoff::Url("javascript:alert(1)".into()))
            .unwrap();
        assert!(matches!(server.join().unwrap(), Err(LinkError::UnsafeUrl)));
    });
}

#[test]
fn file_handoff_name_is_normalised_before_the_receiver_sees_it() {
    let offer = FileOffer {
        id: 1,
        name: "../../etc/passwd".into(),
        size: 3,
        mime: None,
    };
    let (_, got) = exchange(&Handoff::File(offer), ACCEPT);
    let Some(ReceivedHandoff::File(got)) = got.unwrap() else {
        panic!("expected a file")
    };
    assert!(
        !got.name.contains('/') && !got.name.contains(".."),
        "{}",
        got.name
    );
}

#[test]
fn debug_output_never_shows_url_or_file_name() {
    let url = Handoff::Url("https://secret.example/token".into());
    let file = Handoff::File(FileOffer {
        id: 1,
        name: "secret.txt".into(),
        size: 1,
        mime: None,
    });
    let safe = ReceivedHandoff::Url(check_url("https://secret.example/token").unwrap());
    for shown in [format!("{url:?}"), format!("{file:?}"), format!("{safe:?}")] {
        assert!(!shown.contains("secret"), "{shown}");
    }
    assert!(!format!("{:?}", check_url("https://secret.example").unwrap()).contains("secret"));
}
