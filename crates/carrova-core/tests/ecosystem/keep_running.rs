//! Keeping the process running for its links (a phone's link service) and
//! pausing linking, as the link supervisor decides them.

use carrova_core::Event;
use carrova_link::PolicyDoc;
use carrova_platform::LinkNotice;

use crate::supervisor::device::{Device, fast, pair, start_both, wait_until};

/// The last notice `device` asked to keep running with, once it gave one.
fn last_kept(device: &Device) -> Option<Option<LinkNotice>> {
    device.platform.inner.kept_running().last().cloned()
}

fn wait_kept(device: &Device, want: Option<&LinkNotice>) {
    wait_until("keep running notice", || {
        last_kept(device) == Some(want.cloned())
    });
}

fn high_notice(paused: bool) -> Option<LinkNotice> {
    Some(LinkNotice {
        devices: vec!["High".to_owned()],
        paused,
    })
}

#[test]
fn a_device_with_links_keeps_running_and_stops_once_they_are_forgotten() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    let _sl = low.start(fast());

    wait_kept(&low, high_notice(false).as_ref());

    low.forget(&high.id);
    wait_kept(&low, None);
    assert_eq!(
        low.platform.inner.kept_running(),
        [high_notice(false), None],
        "told only on change"
    );
}

#[test]
fn a_device_without_wanted_links_lets_the_service_stop_at_once() {
    let (low, _high, _) = pair(&PolicyDoc::computer_defaults());
    let _sl = low.start(fast());

    wait_kept(&low, None);
    low.source.wait_rounds(5);
    assert_eq!(low.platform.inner.kept_running(), [None]);
}

#[test]
fn pausing_closes_the_links_until_resumed() {
    let (low, high, _) = pair(&PolicyDoc::phone_defaults());
    let (_sl, _sh) = start_both(&low, &high);
    let up = |e: &Event| matches!(e, Event::LinkUp { device } if *device == high.id);
    low.wait("linked", up);

    low.platform.inner.set_links_paused(true);
    low.wait(
        "paused link down",
        |e| matches!(e, Event::LinkDown { device, .. } if *device == high.id),
    );
    wait_kept(&low, high_notice(true).as_ref());
    let ups = low.count(up);
    low.source.wait_rounds(10);
    assert_eq!(low.count(up), ups, "no link while paused");

    low.platform.inner.set_links_paused(false);
    wait_until("linked again", || low.count(up) > ups);
    wait_kept(&low, high_notice(false).as_ref());
}
