use std::sync::Arc;
use std::sync::mpsc::TryRecvError;

use super::{Fanout, SUBSCRIBER_QUEUE};
use crate::session::{Event, EventSink, NullSink};

#[test]
fn a_stalled_subscriber_is_dropped_without_blocking_the_others() {
    let fanout = Fanout::new(Arc::new(NullSink));
    let stalled = fanout.subscribe();
    let live = fanout.subscribe();

    let mut got = 0;
    for _ in 0..SUBSCRIBER_QUEUE + 10 {
        fanout.emit(Event::PairingFailed);
        got += live.try_iter().count();
    }

    assert_eq!(got, SUBSCRIBER_QUEUE + 10);
    assert_eq!(stalled.try_iter().count(), SUBSCRIBER_QUEUE);
    assert_eq!(stalled.try_recv(), Err(TryRecvError::Disconnected));
}
