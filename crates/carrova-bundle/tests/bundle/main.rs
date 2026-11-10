//! Integration tests: build → sign → verify → fetch → install, over a
//! directory and over HTTP, plus source resolution.

// Test helpers outside #[test] fns (same convention as carrova-core tests).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod http;
mod plugins;
mod resolve;
mod store;
mod support;
mod update;
