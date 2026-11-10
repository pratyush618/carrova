//! `BundleStore`: the installed bundle is re-verified against the current
//! roots, rolled-back versions are remembered, `reset` returns to the
//! shipped bundle, and every state change is serialized by a store lock.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[path = "../bundle/support.rs"]
pub mod support;

mod lock;
mod read_only;
mod refused;
mod reset;
mod revoked;
