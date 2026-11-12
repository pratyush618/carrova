#[cfg(unix)]
#[test]
fn stdout_of_keeps_output_in_a_legacy_code_page() {
    use super::*;

    // "dinámico" in CP1252/Latin-1: not valid UTF-8, as ARP.EXE prints it.
    let mut cmd = Command::new("printf");
    cmd.arg(r"din\341mico");
    assert_eq!(stdout_of(&mut cmd).as_deref(), Some("din\u{fffd}mico"));
}
