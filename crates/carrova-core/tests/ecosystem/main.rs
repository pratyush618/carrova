//! Public API of `carrova_core::ecosystem`: device policies, pairing invites
//! and pair-only linking.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/mod.rs"]
mod common;
mod control;
mod invite;
mod keep_running;
#[expect(dead_code, reason = "shared test helpers; this test uses a subset")]
#[path = "../support/lan.rs"]
mod lan_support;
mod pair;
mod phone;
mod policy;
mod supervisor;
