use super::*;

#[test]
fn close_codes_round_trip() {
    for c in CloseCode::ALL {
        assert_eq!(CloseCode::from_wire(c.to_wire()), Some(c));
    }
    assert_eq!(CloseCode::from_wire(VarInt::from_u32(99)), None);
}

#[test]
fn close_codes_distinct() {
    let wires: std::collections::HashSet<u64> = CloseCode::ALL
        .into_iter()
        .map(|c| c.to_wire().into_inner())
        .collect();
    assert_eq!(wires.len(), CloseCode::ALL.len());
}

#[test]
fn link_close_codes_are_appended() {
    assert_eq!(CloseCode::LinkFailed.to_wire(), VarInt::from_u32(5));
    assert_eq!(CloseCode::Superseded.to_wire(), VarInt::from_u32(6));
}
