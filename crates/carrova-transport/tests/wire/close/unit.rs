use super::*;

#[test]
fn close_codes_round_trip() {
    for c in CloseCode::ALL {
        assert_eq!(CloseCode::from_wire(c.to_wire()), Some(c));
    }
    assert_eq!(CloseCode::from_wire(VarInt::from_u32(99)), None);
}
