//! Host imports (`carrova.log`, `carrova.run_tool`): linked only when the
//! manifest declares them, capped, and never more than declared.

#![allow(clippy::unwrap_used, clippy::panic)]

#[path = "../support/mod.rs"]
mod support;

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};

use carrova_platform::OsKind;
use carrova_platform::fake::FakePlatform;
use carrova_plugin::{Plugin, PluginError, PluginManifest, RunBudget, TransformContext};
use support::{ALLOC, load, manifest};

/// A module importing `carrova.<name>` with `sig`, whose `transform` runs
/// `body`.
fn importing(name: &str, sig: &str, body: &str) -> String {
    format!(
        r#"(module (import "carrova" "{name}" (func ${name} {sig})) {ALLOC}
  (func (export "transform") (param i32 i32 i32 i32 i32 i32) (result i64) {body}))"#
    )
}

const LOG_SIG: &str = "(param i32 i32)";
const TOOL_SIG: &str = "(param i32 i32 i32 i32) (result i64)";

/// Logs the file's bytes, returns nothing.
fn logger() -> String {
    importing(
        "log",
        LOG_SIG,
        "(call $log (local.get 4) (local.get 5)) (i64.const 0)",
    )
}

/// Runs the tool named by the file's path on the file's bytes and returns
/// its output.
fn tool_runner() -> String {
    importing(
        "run_tool",
        TOOL_SIG,
        "(call $run_tool (local.get 2) (local.get 3) (local.get 4) (local.get 5))",
    )
}

fn with_log() -> PluginManifest {
    PluginManifest {
        log: true,
        ..manifest()
    }
}

fn with_tools(tools: &[&str]) -> PluginManifest {
    PluginManifest {
        run_tools: tools.iter().map(|&t| t.to_owned()).collect(),
        ..manifest()
    }
}

fn ctx() -> TransformContext {
    TransformContext {
        source_home: "/home/ada".into(),
        target_home: "/home/ada".into(),
        source_os: OsKind::Linux,
        target_os: OsKind::Linux,
    }
}

fn run(
    plugin: &Plugin,
    platform: &FakePlatform,
    path: &str,
    bytes: &[u8],
    budget: &mut RunBudget,
) -> Result<Vec<u8>, PluginError> {
    plugin.transform(platform, path, bytes, &ctx(), budget)
}

fn no_platform() -> FakePlatform {
    FakePlatform::with_home(std::env::temp_dir())
}

#[test]
fn host_imports_are_linked_only_when_declared() {
    let refused = |wat: &str, m: PluginManifest, want: &str| match load(wat, m) {
        Err(PluginError::Import { module, name }) => {
            assert_eq!((module.as_str(), name.as_str()), ("carrova", want));
        }
        other => panic!("{want}: expected an import error, got {other:?}"),
    };
    refused(&logger(), manifest(), "log");
    refused(&tool_runner(), with_log(), "run_tool");
    load(&logger(), with_log()).unwrap();
    load(&tool_runner(), with_tools(&["jq"])).unwrap();
    let everything = PluginManifest {
        log: true,
        run_tools: vec!["jq".into()],
        ..manifest()
    };
    refused(
        &importing("open", LOG_SIG, "(i64.const 0)"),
        everything,
        "open",
    );
}

/// Debug events of the `plugin` target recorded while `f` runs.
fn captured_log(f: impl FnOnce()) -> String {
    #[derive(Clone, Default)]
    struct Buf(Arc<Mutex<Vec<u8>>>);
    impl Write for Buf {
        fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .extend_from_slice(data);
            Ok(data.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let buf = Buf::default();
    let writer = buf.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_ansi(false)
        .with_writer(move || writer.clone())
        .finish();
    tracing::subscriber::with_default(subscriber, f);
    let bytes = buf.0.lock().unwrap_or_else(PoisonError::into_inner).clone();
    String::from_utf8(bytes).unwrap()
}

#[test]
fn log_lines_are_debug_events_tagged_with_the_plugin_and_sanitised() {
    let plugin = load(&logger(), with_log()).unwrap();
    let log = captured_log(|| {
        let line = "vault \u{1b}[2Jmoved \u{202e}here\n";
        run(
            &plugin,
            &no_platform(),
            "f",
            line.as_bytes(),
            &mut RunBudget::new(),
        )
        .unwrap();
    });
    let line = log.lines().next().unwrap();
    assert!(line.contains("DEBUG"), "{log}");
    assert!(line.contains("plugin:"), "{log}");
    assert!(line.contains("plugin=test"), "{log}");
    assert!(line.contains("vault"), "{log}");
    assert!(line.contains("here"), "{log}");
    assert!(
        !log.contains('\u{1b}') && !log.contains('\u{202e}'),
        "{log:?}"
    );
    assert_eq!(log.lines().count(), 1, "{log:?}");
}

#[test]
fn log_is_capped_per_call_and_per_run_with_one_marker() {
    let plugin = load(&logger(), with_log()).unwrap();
    let mut budget = RunBudget::new();
    let log = captured_log(|| {
        let long = "x".repeat(carrova_plugin::MAX_LOG_LINE + 500);
        run(&plugin, &no_platform(), "f", long.as_bytes(), &mut budget).unwrap();
        for _ in 0..40 {
            run(&plugin, &no_platform(), "f", &[b'y'; 1000], &mut budget).unwrap();
        }
    });
    let first = log.lines().next().unwrap();
    assert!(
        first.contains(&"x".repeat(carrova_plugin::MAX_LOG_LINE)),
        "{first}"
    );
    assert!(
        !first.contains(&"x".repeat(carrova_plugin::MAX_LOG_LINE + 1)),
        "{first}"
    );
    let payload: usize = log.matches(['x', 'y']).count();
    assert!(payload <= carrova_plugin::MAX_RUN_LOG, "{payload}");
    assert!(payload > carrova_plugin::MAX_RUN_LOG - 1000, "{payload}");
    assert_eq!(log.matches("truncated").count(), 1, "{log}");
    assert!(log.lines().last().unwrap().contains("truncated"), "{log}");
}

#[cfg(unix)]
mod tools {
    use std::path::Path;
    use std::time::{Duration, Instant};

    use carrova_platform::fake::{FakePlatform, write_tool_script};
    use carrova_plugin::{MAX_IO_BYTES, PluginError, RunBudget};
    use tempfile::TempDir;

    use super::{load, run, tool_runner, with_tools};

    /// A platform where each `(name, sh body)` is an installed tool.
    fn platform(dir: &Path, tools: &[(&str, &str)]) -> FakePlatform {
        tools
            .iter()
            .fold(FakePlatform::with_home(dir), |p, (name, body)| {
                p.binary(*name, write_tool_script(dir, name, body).unwrap())
            })
    }

    fn trap(result: Result<Vec<u8>, PluginError>) -> String {
        match result {
            Err(PluginError::Trap(why)) => why,
            other => panic!("expected a trap, got {other:?}"),
        }
    }

    #[test]
    fn a_declared_tool_gets_the_input_on_stdin_and_no_arguments() {
        let tmp = TempDir::new().unwrap();
        let p = platform(
            tmp.path(),
            &[("upper", "printf '%s:' \"$#\"; tr a-z A-Z\n")],
        );
        let plugin = load(&tool_runner(), with_tools(&["upper"])).unwrap();
        let out = run(&plugin, &p, "upper", b"hello", &mut RunBudget::new()).unwrap();
        assert_eq!(out, b"0:HELLO");
    }

    #[test]
    fn an_undeclared_tool_traps_without_running() {
        let tmp = TempDir::new().unwrap();
        let marker = tmp.path().join("ran");
        let body = format!("touch '{}'\n", marker.display());
        let p = platform(tmp.path(), &[("upper", "cat\n"), ("other", &body)]);
        let plugin = load(&tool_runner(), with_tools(&["upper"])).unwrap();
        let why = trap(run(&plugin, &p, "other", b"x", &mut RunBudget::new()));
        assert!(why.contains("not declared"), "{why}");
        assert!(!marker.exists());
        let why = trap(run(&plugin, &p, "/bin/sh", b"x", &mut RunBudget::new()));
        assert!(why.contains("not declared"), "{why}");
    }

    #[test]
    fn a_missing_or_failing_tool_traps() {
        let tmp = TempDir::new().unwrap();
        let p = platform(tmp.path(), &[("fails", "echo secret-output; exit 3\n")]);
        let plugin = load(&tool_runner(), with_tools(&["fails", "absent"])).unwrap();
        let why = trap(run(&plugin, &p, "absent", b"", &mut RunBudget::new()));
        assert!(why.contains("not installed"), "{why}");
        let why = trap(run(&plugin, &p, "fails", b"", &mut RunBudget::new()));
        assert!(why.contains("exit"), "{why}");
        assert!(!why.contains("secret-output"), "{why}");
    }

    #[test]
    fn a_tool_running_past_its_time_is_killed() {
        let tmp = TempDir::new().unwrap();
        let p = platform(tmp.path(), &[("slow", "exec sleep 30\n")]);
        let plugin = load(&tool_runner(), with_tools(&["slow"])).unwrap();
        let mut budget = RunBudget::new().with_tool_time(Duration::from_millis(300));
        let start = Instant::now();
        let why = trap(run(&plugin, &p, "slow", b"", &mut budget));
        assert!(why.contains("too long"), "{why}");
        assert!(start.elapsed() < Duration::from_secs(10));
        let why = trap(run(&plugin, &p, "slow", b"", &mut budget));
        assert!(why.contains("time"), "{why}");
    }

    /// Whether process `pid` still exists (a reaped zombie counts as gone).
    fn alive(pid: &str) -> bool {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    }

    #[test]
    fn a_timed_out_tool_takes_its_children_with_it() {
        let tmp = TempDir::new().unwrap();
        let pid_file = tmp.path().join("grandchild.pid");
        // The background `sleep` inherits stdout, so it holds the pipe open.
        let body = format!("sleep 30 &\necho $! > '{}'\nwait\n", pid_file.display());
        let p = platform(tmp.path(), &[("spawner", &body)]);
        let plugin = load(&tool_runner(), with_tools(&["spawner"])).unwrap();
        let mut budget = RunBudget::new().with_tool_time(Duration::from_millis(500));
        let start = Instant::now();
        let why = trap(run(&plugin, &p, "spawner", b"", &mut budget));
        assert!(why.contains("too long"), "{why}");
        assert!(start.elapsed() < Duration::from_secs(5));
        let pid = std::fs::read_to_string(&pid_file).unwrap();
        let pid = pid.trim();
        // The killed grandchild is reaped by init asynchronously.
        let deadline = Instant::now() + Duration::from_secs(3);
        while alive(pid) && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(pid), "grandchild {pid} outlived the tool timeout");
    }

    #[test]
    fn tool_output_beyond_the_cap_traps() {
        let tmp = TempDir::new().unwrap();
        let body = format!("head -c {} /dev/zero\n", MAX_IO_BYTES + 1);
        let p = platform(tmp.path(), &[("huge", &body)]);
        let plugin = load(&tool_runner(), with_tools(&["huge"])).unwrap();
        let why = trap(run(&plugin, &p, "huge", b"", &mut RunBudget::new()));
        assert!(why.contains("too large"), "{why}");
    }
}
