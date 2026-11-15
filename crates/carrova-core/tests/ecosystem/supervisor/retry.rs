//! Sending a file again after its device left mid-offer.

use std::thread;

use carrova_core::ecosystem::{Answer, FileFailure};
use carrova_core::{CoreError, Event};
use carrova_link::{Feature, FeatureState, PolicyDoc};

use super::device::{fast, wait_until};
use super::{asks, link_up, linked, wait_remote};

#[test]
fn a_file_whose_device_left_can_be_sent_again() {
    let (low, high, sl, sh) = linked(&PolicyDoc::phone_defaults());
    let src = low.home.join("notes.txt");
    std::fs::write(&src, b"draft").unwrap();

    thread::scope(|s| {
        let sending = s.spawn(|| sl.send_file(&high.id, &src));
        high.wait("ask", |e| matches!(e, Event::AskReceived { .. }));
        sh.stop();
        assert!(sending.join().unwrap().is_err());
    });
    let Event::FileFailed { id, .. } = low.wait("left", |e| {
        matches!(
            e,
            Event::FileFailed {
                reason: FileFailure::DeviceLeft,
                retry: true,
                ..
            }
        )
    }) else {
        unreachable!()
    };

    let sh = high.start(fast());
    low.sees(&high, &sh);
    high.sees(&low, &sl);
    wait_until("linked again", || low.count(link_up(&high.id)) >= 2);
    wait_remote(&sl, &high.id, Feature::Files, FeatureState::Ask);

    thread::scope(|s| {
        let again = s.spawn(|| sl.retry_file(id));
        wait_until("the second ask", || asks(&high).len() == 2);
        let (ask, ..) = asks(&high).pop().unwrap();
        sh.answer(ask, Answer::Accept).unwrap();
        again.join().unwrap().unwrap();
    });
    let saved = high.home.join("Downloads").join("notes.txt");
    assert_eq!(std::fs::read(saved).unwrap(), b"draft");
    assert!(
        matches!(sl.retry_file(id), Err(CoreError::NothingToRetry)),
        "sent: nothing left to retry"
    );
}

#[test]
fn a_refused_file_is_not_offered_for_retry() {
    let (low, high, sl, sh) = linked(&PolicyDoc::phone_defaults());
    let src = low.home.join("no.txt");
    std::fs::write(&src, b"no").unwrap();

    thread::scope(|s| {
        let sending = s.spawn(|| sl.send_file(&high.id, &src));
        high.wait("ask", |e| matches!(e, Event::AskReceived { .. }));
        let (ask, ..) = asks(&high).pop().unwrap();
        sh.answer(ask, Answer::Decline).unwrap();
        assert!(sending.join().unwrap().is_err());
    });
    let Event::FileFailed { id, retry, .. } =
        low.wait("refused", |e| matches!(e, Event::FileFailed { .. }))
    else {
        unreachable!()
    };
    assert!(!retry);
    assert!(matches!(sl.retry_file(id), Err(CoreError::NothingToRetry)));
}
