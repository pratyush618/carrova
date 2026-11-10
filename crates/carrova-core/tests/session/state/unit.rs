use super::State::*;

#[test]
fn transitions() {
    assert!(Scanned.can_transition(Selected));
    assert!(Applying.can_transition(RolledBack));
    assert!(Done.can_transition(RolledBack));
    assert!(Transferring.can_transition(Planned));
    assert!(!Scanned.can_transition(Applying));
    assert!(!RolledBack.can_transition(Done));
    assert!(!Failed.can_transition(Scanned));
    assert!(!Done.can_transition(Failed));
}

#[test]
fn names_shown_to_users_pad_like_strings() {
    let (state, role) = (RolledBack, super::Role::Export);
    assert_eq!(format!("{state:<12}|"), "rolled back |");
    assert_eq!(format!("{role:<6}|"), "export|");
    assert_eq!(Transferring.to_string(), "transferring");
}

#[test]
fn id_shape() {
    assert!(super::SessionId::is_valid("20261108-101500-a1b2c3"));
    assert!(!super::SessionId::is_valid("../x"));
    assert!(!super::SessionId::is_valid(""));
}
