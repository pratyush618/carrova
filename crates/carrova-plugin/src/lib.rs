//! Carrova WASM plugins: sandboxed, receiver-side content transforms for
//! apps whose data needs logic to move (e.g. config files holding absolute
//! paths).
//!
//! # Model
//!
//! A recipe item names a plugin (`handler: { plugin: <id> }`, recipe
//! schema 3). On the receiver, every file of that item is passed through
//! [`Plugin::transform`] after staging verified it; the output is staged,
//! verified and applied by the normal journalled apply, so a plugin never
//! writes a file itself. The user confirms each plugin per import after
//! seeing [`PluginManifest::permissions`].
//!
//! Plugins ship only inside the signed recipe bundle (core-maintainer
//! review, signed releases); the developer recipe override may load
//! unsigned ones, with the usual warning.
//!
//! # ABI (core WebAssembly, no WASI)
//!
//! The module imports **nothing** (any import is refused at load) and
//! exports:
//!
//! | Export | Type | Role |
//! |--------|------|------|
//! | `memory` | memory | its only linear memory |
//! | `alloc` | `(len: i32) -> i32` | a buffer of `len` bytes the host fills |
//! | `transform` | `(ctx_ptr, ctx_len, path_ptr, path_len, data_ptr, data_len: i32) -> i64` | the new content |
//!
//! The host calls `alloc` three times and writes the context, the file's
//! target path relative to its semantic root (UTF-8, `/`-separated, e.g.
//! `obsidian/obsidian.json`) and the file's bytes. The context is `key=value\n` per [`ContextField`] the manifest
//! declares, in declared order, nothing else. `transform` returns
//! `(out_ptr << 32) | out_len`, or a negative status to refuse the file
//! ([`PluginError::Refused`]). Each call gets a fresh instance: no state
//! survives between files.
//!
//! # Limits
//!
//! Per call: the manifest's `limits.fuel` (≤ [`MAX_FUEL`]) and
//! `limits.memory_mib` (≤ [`MAX_MEMORY_MIB`]); one memory, one table, one
//! instance. Per plugin and restore: [`MAX_RUN_FUEL`] over all its files.
//! Input and output ≤ [`MAX_IO_BYTES`], module ≤
//! [`MAX_WASM_BYTES`]. Modules are validated eagerly at load.
//!
//! # Threat model
//!
//! A plugin is untrusted code that may be buggy or malicious. The sandbox
//! is structural: with no imports it has no way to reach files, the
//! network, processes, clocks or randomness; it only sees what the host
//! copies in (one file, its relative path, the declared context) and can
//! only answer with bytes. Fuel and memory limits bound denial of service.
//! What it *can* do is write anything into the files of the items that use
//! it — the user's consent covers exactly that, and the result still goes
//! through path confinement, the dry-run plan and reversible apply.
//!
//! shortcut: receiver-side transforms only; sender-side export (reading an
//! app database) and calling the app's own export tool need new
//! capabilities and host functions — add them when a recipe needs one.

mod error;
mod manifest;
mod runtime;

pub use error::PluginError;
pub use manifest::{
    ContextField, Limits, MANIFEST_SCHEMA, MAX_FUEL, MAX_MEMORY_MIB, PluginManifest,
};
pub use runtime::{
    MAX_IO_BYTES, MAX_RUN_FUEL, MAX_WASM_BYTES, Plugin, PluginPackage, TransformContext,
};
