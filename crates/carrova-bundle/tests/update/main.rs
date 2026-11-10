//! `Updater` admission policy beyond plain rollback protection: bounded
//! version jumps, monotonic creation time, and the release pointer.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../bundle/support.rs"]
pub mod support;

mod pointer;
mod policy;
