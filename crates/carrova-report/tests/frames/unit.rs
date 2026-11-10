use super::*;

const RENDERED: &str = "   0: std::backtrace::Backtrace::force_capture
             at /rustc/abc/library/std/src/backtrace.rs:312:13
   1: carrova_report::hook::install::{{closure}}::h0123456789abcdef
             at ./crates/carrova-report/src/hook.rs:40:9
   2: <carrova_core::engine::Exporter as core::fmt::Debug>::fmt
   3: carrova_core::engine::Exporter::run
   4: carrova_core::engine::Exporter::run
   5: tokio::runtime::park::wait
             at /home/ana/.cargo/registry/src/tokio/src/park.rs:1:1
   6: main";

#[test]
fn keeps_only_carrova_symbols_without_paths_or_hashes() {
    assert_eq!(
        our_frames(RENDERED),
        [
            "carrova_report::hook::install::{{closure}}",
            "<carrova_core::engine::Exporter as core::fmt::Debug>::fmt",
            "carrova_core::engine::Exporter::run",
        ]
    );
}

#[test]
fn disabled_backtrace_yields_no_frames() {
    assert!(our_frames("disabled backtrace").is_empty());
}
