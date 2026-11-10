use std::net::Ipv4Addr;

use super::*;

fn ip(n: u8) -> IpAddr {
    IpAddr::V4(Ipv4Addr::new(10, 0, 0, n))
}

fn admitted(a: Admission) -> Slot {
    match a {
        Admission::Admit(slot) => slot,
        other => panic!("expected a slot, got {other:?}"),
    }
}

fn refusal(a: &Admission) -> Option<Refusal> {
    match a {
        Admission::Refuse(r) => Some(*r),
        _ => None,
    }
}

#[test]
fn slots_are_limited_per_address_and_in_total_and_freed_on_drop() {
    let slots = HandshakeSlots::new(2);
    let first = admitted(slots.reserve(ip(1)));
    assert_eq!(refusal(&slots.reserve(ip(1))), Some(Refusal::AddressBusy));
    let second = admitted(slots.reserve(ip(2)));
    assert_eq!(refusal(&slots.reserve(ip(3))), Some(Refusal::Full));
    drop(first);
    let again = admitted(slots.reserve(ip(1)));
    drop((second, again));
    assert!(slots.pending.lock().unwrap().is_empty());
}
