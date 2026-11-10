//! Carrova WASM plugins: sandboxed content transforms for apps whose data
//! needs logic to move (e.g. config files holding absolute paths).
//!
//! # Model
//!
//! A recipe item names a plugin (`handler: { plugin: <id> }`, recipe
//! schema 3). On the receiver, every file of that item is passed through
//! [`Plugin::transform`] after staging verified it; the output is staged,
//! verified and applied by the normal journalled apply, so a plugin never
//! writes a file itself. A plugin declaring `export` also runs on the
//! sender: [`Plugin::export`] rewrites each file of its items before it is
//! chunked, and the output is what is sent. The user confirms each plugin
//! (per import, and per send for `export`) after seeing
//! [`PluginManifest::permissions`].
//!
//! Plugins ship only inside the signed recipe bundle (core-maintainer
//! review, signed releases); the developer recipe override may load
//! unsigned ones, with the usual warning.
//!
//! # ABI (core WebAssembly, no WASI)
//!
//! The module exports:
//!
//! | Export | Type | Role |
//! |--------|------|------|
//! | `memory` | memory | its only linear memory |
//! | `alloc` | `(len: i32) -> i32` | a buffer of `len` bytes the host fills |
//! | `transform` | `(ctx_ptr, ctx_len, path_ptr, path_len, data_ptr, data_len: i32) -> i64` | the new content (receiver) |
//! | `export_transform` | same as `transform` | the content to send (sender); exported exactly when the manifest declares `export: true` |
//!
//! The host calls `alloc` three times and writes the context, the file's
//! path relative to its semantic root (UTF-8, `/`-separated, e.g.
//! `obsidian/obsidian.json`) and the file's bytes. The context is
//! `key=value\n` per [`ContextField`] the manifest declares, in declared
//! order, nothing else; `export_transform` gets only `source_home` and
//! `source_os` (this computer's). The entry point returns
//! `(out_ptr << 32) | out_len`, or a negative status to refuse the file
//! ([`PluginError::Refused`]). Each call gets a fresh instance: no state
//! survives between files.
//!
//! The module imports nothing but the host functions its manifest declares
//! (import module `carrova`, see `host.rs`); anything else is refused at
//! load:
//!
//! | Import | Type | Declared by |
//! |--------|------|-------------|
//! | `log` | `(ptr, len: i32)` | `log: true` |
//! | `run_tool` | `(name_ptr, name_len, in_ptr, in_len: i32) -> i64` | `run_tools: [names]` |
//!
//! `run_tool` runs a declared program with **no arguments**, the input as
//! its stdin, and returns `(ptr << 32) | len` of its stdout, copied into a
//! buffer from the plugin's `alloc`. Misuse (an undeclared name, a buffer
//! outside memory, a missing, failing or slow tool) traps.
//!
//! # Limits
//!
//! Per call: the manifest's `limits.fuel` (≤ [`MAX_FUEL`]) and
//! `limits.memory_mib` (≤ [`MAX_MEMORY_MIB`]); one memory, one table, one
//! instance. Per plugin and run (a restore, or a capture), a
//! [`RunBudget`]: [`MAX_RUN_FUEL`] over all its files, [`MAX_RUN_LOG`] log
//! bytes, [`MAX_RUN_TOOL_TIME`] of tools running. Input and output ≤
//! [`MAX_IO_BYTES`] (tool output too), module ≤ [`MAX_WASM_BYTES`], log
//! line ≤ [`MAX_LOG_LINE`], tool call ≤ [`TOOL_TIMEOUT`]. Modules are
//! validated eagerly at load.
//!
//! # Threat model
//!
//! A plugin is untrusted code that may be buggy or malicious. The sandbox
//! is structural: without host imports it has no way to reach files, the
//! network, processes, clocks or randomness; it only sees what the host
//! copies in (one file, its relative path, the declared context) and can
//! only answer with bytes. Fuel, memory and the run budget bound denial of
//! service. What it *can* do is write anything into the files of the items
//! that use it — the user's consent covers exactly that, and the result
//! still goes through path confinement, the dry-run plan and reversible
//! apply.
//!
//! Each host import widens that, and only when declared and consented to:
//!
//! - `log`: plugins see file contents, so a log line may carry some. Lines
//!   go only to `tracing` at **debug** level (target `plugin`, field
//!   `plugin` = id), cut to [`MAX_LOG_LINE`], stripped of control and bidi
//!   characters, and capped per run; past the cap one "truncated" marker is
//!   written and the rest dropped.
//! - `run_tool`: a declared program runs **outside the sandbox** with the
//!   user's rights. The plugin picks only which declared program (a bare
//!   name, validated in the manifest and resolved through the platform's
//!   `PATH` snapshot) and what it reads on stdin; it passes no arguments,
//!   the environment is the platform's minimal tool environment, stderr is
//!   discarded and the tool is killed past its time. The consent text
//!   names every program and says it is not sandboxed.
//! - `export`: the plugin also runs on the sender, over the files of its
//!   items only, before they leave; the receiver still checks everything
//!   it gets as untrusted.

mod budget;
mod error;
mod host;
mod manifest;
mod runtime;
mod tool;

pub use budget::{MAX_RUN_FUEL, MAX_RUN_LOG, MAX_RUN_TOOL_TIME, RunBudget};
pub use error::PluginError;
pub use host::MAX_LOG_LINE;
pub use manifest::{
    ContextField, Limits, MANIFEST_SCHEMA, MAX_FUEL, MAX_MEMORY_MIB, PluginManifest,
};
pub use runtime::{MAX_IO_BYTES, MAX_WASM_BYTES, Plugin, PluginPackage, TransformContext};
pub use tool::TOOL_TIMEOUT;
