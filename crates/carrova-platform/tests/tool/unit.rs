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
    assert!(started.elapsed() < Duration::from_secs(2));
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

/// Whether `pid` is a live process; a zombie awaiting reaping is dead.
#[cfg(unix)]
fn alive(pid: &str) -> bool {
    let ps = Command::new("ps")
        .args(["-o", "stat=", "-p", pid])
        .output()
        .unwrap();
    let stat = String::from_utf8_lossy(&ps.stdout);
    !stat.trim().is_empty() && !stat.trim_start().starts_with('Z')
}

#[cfg(unix)]
#[test]
fn a_timeout_kills_the_tools_children_too() {
    let pid_file = std::env::temp_dir().join(format!("carrova-tool-{}", std::process::id()));
    let script = format!("sleep 30 & echo $! > {}; wait", pid_file.display());
    // The limit is long enough for the shell to have written the pid.
    assert_eq!(
        run_bounded(&mut sh(&script), Duration::from_millis(1500), 1024),
        None
    );
    let pid = std::fs::read_to_string(&pid_file).unwrap();
    let _ = std::fs::remove_file(&pid_file);
    let pid = pid.trim();
    assert!(!pid.is_empty(), "the shell never wrote the grandchild pid");
    // The kill is synchronous; init may need a moment to reap the orphan.
    let until = Instant::now() + Duration::from_secs(2);
    while alive(pid) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(!alive(pid), "grandchild {pid} survived");
}

#[cfg(unix)]
#[test]
fn a_tool_that_exited_unsuccessfully_is_not_killed_afterwards() {
    let mut cmd = sh("exit 3");
    crate::sys::isolate_tool(&mut cmd);
    let mut child = cmd.stdout(Stdio::piped()).spawn().unwrap();
    // Reaped by the status check: signalling its pid now could hit a stranger.
    assert!(matches!(
        read_within(&mut child, Duration::from_secs(5), 1024),
        Run::Exited
    ));
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
