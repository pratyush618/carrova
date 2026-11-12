use super::*;
use std::time::Instant;

#[cfg(unix)]
fn sh(script: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", script]);
    cmd
}

#[cfg(unix)]
#[test]
fn stdout_of_keeps_output_in_a_legacy_code_page() {
    // "dinámico" in CP1252/Latin-1: not valid UTF-8, as ARP.EXE prints it.
    let mut cmd = Command::new("printf");
    cmd.arg(r"din\341mico");
    assert_eq!(stdout_of(&mut cmd).as_deref(), Some("din\u{fffd}mico"));
}

#[cfg(unix)]
#[test]
fn a_tool_that_runs_past_the_deadline_is_killed_and_yields_none() {
    let started = Instant::now();
    let out = run_bounded(&mut sh("sleep 30"), Duration::from_millis(200), 1024);
    assert_eq!(out, None);
    assert!(started.elapsed() < Duration::from_secs(1));
}

#[cfg(unix)]
#[test]
fn output_past_the_cap_yields_none() {
    let out = run_bounded(&mut Command::new("yes"), Duration::from_secs(5), 1024);
    assert_eq!(out, None);
}

#[cfg(unix)]
#[test]
fn output_within_the_cap_is_returned() {
    let out = run_bounded(&mut sh("printf ok"), Duration::from_secs(5), 1024);
    assert_eq!(out.as_deref(), Some("ok"));
}

#[cfg(unix)]
#[test]
fn a_failing_tool_yields_none() {
    let out = run_bounded(&mut sh("echo hi; exit 3"), Duration::from_secs(5), 1024);
    assert_eq!(out, None);
}

#[cfg(unix)]
#[test]
fn a_timeout_kills_the_tools_children_too() {
    let pid_file = std::env::temp_dir().join(format!("carrova-tool-{}", std::process::id()));
    let script = format!("sleep 30 & echo $! > {}; wait", pid_file.display());
    assert_eq!(
        run_bounded(&mut sh(&script), Duration::from_millis(500), 1024),
        None
    );
    let pid = std::fs::read_to_string(&pid_file).unwrap();
    let _ = std::fs::remove_file(&pid_file);
    let alive = || {
        Command::new("kill")
            .args(["-0", pid.trim()])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    };
    // The kill is synchronous; init may need a moment to reap the orphan.
    let until = Instant::now() + Duration::from_secs(2);
    while alive() && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(), "grandchild {pid} survived");
}

#[cfg(windows)]
#[test]
fn a_slow_tool_is_killed_at_the_deadline() {
    let mut cmd = Command::new("cmd");
    cmd.args(["/c", "ping -n 20 127.0.0.1"]);
    let started = Instant::now();
    assert_eq!(
        run_bounded(&mut cmd, Duration::from_millis(500), 1024),
        None
    );
    assert!(started.elapsed() < Duration::from_secs(5));
}
