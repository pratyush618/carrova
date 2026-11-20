use carrova_view::ipc::ErrorCode;

use super::*;

#[test]
fn a_second_start_is_refused_while_the_first_runs() {
    let flight = InFlight::default();
    let first = flight.start("the update").unwrap();
    let second = flight.start("the update").unwrap_err();
    assert_eq!(second.bridge().code, ErrorCode::Session);
    drop(first);
    assert!(flight.start("the update").is_ok());
}
