# Phase 2 follow-ups (known devices / Nearby / Wake) — plan

Spec: docs/superpowers/specs/2026-11-12-background-nearby-design.md (main worktree,
untracked: /home/ezio/Desktop/Work/personal/porter/docs/superpowers/specs/2026-11-12-background-nearby-design.md)
ADR (binding): docs/adr/0001-known-devices.md. Mockup: /home/ezio/Desktop/Work/personal/porter/docs/mockups/gui-v4.html (grep, don't read whole).
Source: tasks/todo.md, section "Background + Nearby — Phase 2", follow-up items.

## Global Constraints

- Worktree: /home/ezio/Desktop/Work/personal/porter-wt/p2-followups, branch feat/p2-followups. Never force-push, reset --hard or git clean. Don't push.
- CLAUDE.md + tasks/lessons.md rules bind. Boundary rule 1: no OS-specific code outside `carrova-platform`. Rule 2: src-tauri has no business logic.
- `unsafe_code = "forbid"`, no FFI. Libraries: thiserror, no unwrap/expect/panic outside tests.
- Never log MAC addresses, file contents, secrets.
- Tests outside `src/`: `tests/<module>/unit.rs` wired via `#[cfg(test)] #[path = "…"] mod tests;`, integration in `tests/<module>/main.rs`. `just test-layout` enforces.
- Bug fix / new logic → test first, proven failing.
- Ponytail: smallest complete change; reuse existing helpers (`Platform::tool_command`, `crate::sys::tool_command`, `crate::device::sanitize_device_name`, `crate::netops::host_name`). Resolved `shortcut:` comments are removed; any new known limit gets a `shortcut: <limit>, <when to upgrade>` comment.
- Heavy jobs only via `systemd-run --user --scope -p MemoryMax=4G --nice=10 …`, CARGO_BUILD_JOBS=3, shared CARGO_TARGET_DIR=/home/ezio/Desktop/Work/personal/porter/target: `touch crates/<crate>/src/lib.rs` before testing an edited crate. Never run full `just ci` (controller does).
- Cross-target checks available locally: `cargo check/clippy -p carrova-platform --target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin` (core can't build msvc: ring).
- Commits: Conventional, scopes core|recipe|manifest|transport|crypto|archive|platform|plugin|view|bundle|report|cli|desktop|tauri|xtask|recipes|e2e|ci|deps|docs|release|repo; subject ≤60 chars, imperative; split into focused commits; NO AI attribution / Co-Authored-By lines.

## Task 1: Nearby map — no device under the pill or card

Files: apps/desktop/src/screens/nearby/{NearbyMap.tsx,nearby.css}, apps/desktop/e2e/flows/nearby.spec.ts (+ tests/screens/nearby if spot math changes).

- Bug: at a 1100×520 window an outer-ring device sits under the visibility pill (VisibilityPill). Adjust orbit placement (RINGS radius/start, FLATTEN, or clamp to the field) so no device button overlaps the visibility pill or the device card at any supported size (min window width 720).
- Extend e2e/flows/nearby.spec.ts: at viewports 1100×520 and 720×560, with the map's fixture devices showing (both rings), assert every device button's bounding box does not intersect the pill's box nor the card's box (when the card is shown). Test must fail before the fix.
- Visual check vs the mockup's Nearby screen; save screenshots at 1100×520 and 720×560 to /tmp/carrova-nearby-1100x520.png and /tmp/carrova-nearby-720x560.png.
- Out of scope: the 12-device density handling (stays a follow-up in todo.md).
- Verify: `just ui-lint`, `just ui-test`, `just e2e-ui` (capped unit).
- Commit scope: desktop / e2e.

## Task 2: macOS/Windows wake hints + macOS Computer Name (system tools)

Files: crates/carrova-platform/src/{platform.rs,linux/neighbours.rs (move/share),macos/mod.rs,windows/mod.rs}, new shared parsers module, tests.

- `neighbour_mac` on macOS: run `/usr/sbin/arp -an` via the adapter's `tool_command` (minimal env, no shell), parse stdout for the entry of the IPv4 `addr`. Format: `? (192.168.1.1) at 0:1b:2c:d:e:ff on en0 ifscope [ethernet]` (macOS drops leading zeros per byte); `(incomplete)` entries → None. IPv6 → None.
- `neighbour_mac` on Windows: run `%SystemRoot%\System32\ARP.EXE -a` (SystemRoot env var, fallback `C:\Windows`) via `tool_command`; parse lines `  192.168.1.1           aa-bb-cc-dd-ee-ff     dynamic` (header lines may be localized: parse by fields, skip anything else). All-zero / broadcast `ff-ff-ff-ff-ff-ff` → None.
- Parsers are pure functions over `&str` + `Ipv4Addr`, compiled where their tests can run on Linux (shared module beside the Linux `/proc/net/arp` parser; share `parse_mac` with separator param). Fixture tests per OS format incl. incomplete, other IPs, malformed lines, zero MAC. Avoid dead-code warnings per target without `#[allow]` (cfg(any(test, target_os=…)) on each fn is fine).
- Tool failure / non-zero exit / bad UTF-8 → None. Never log MACs (log nothing or the error only).
- `device_name` on macOS: run `/usr/sbin/scutil --get ComputerName` via `tool_command`; success + non-empty trimmed stdout → sanitize_device_name; otherwise fall back to the current host name (strip `.local`). Pure parse fn + test (ok output, trailing newline, empty, failure → fallback).
- Remove the resolved `shortcut:` comments (platform.rs neighbour_mac, macos/mod.rs device_name); update trait doc text that says "Linux: …" only where needed.
- Verify: `cargo test -p carrova-platform`, `cargo clippy -p carrova-platform --all-targets -- -D warnings` for host, `--target x86_64-pc-windows-msvc` and `--target aarch64-apple-darwin`.
- Commit scope: platform (split: shared parsers / macOS arp / Windows arp / macOS name).

## Task 3: macOS/Windows file watch via `notify` (user chose option a)

Files: Cargo.toml ([workspace.dependencies]), crates/carrova-platform/{Cargo.toml,src/platform.rs,src/macos/mod.rs,src/windows/mod.rs, new watch module}, apps/desktop/src-tauri/src/tray.rs (remove shortcut comment), crates/carrova-core/src/presence.rs (docs only if they say mac/win poll), tests.

- Add `notify` (latest stable, default-features as needed for FSEvents on macOS and ReadDirectoryChangesW on Windows; no `serde`), as a dependency of carrova-platform ONLY under `[target.'cfg(any(target_os = "macos", windows))'.dependencies]`.
- Implement `watch_dirs` on macOS and Windows with the exact contract of the trait doc (platform.rs ~line 241): non-recursive; `changed` called on a thread the watch owns when an entry is created/replaced/written/removed; dir missing now → watched from when it appears in another watched dir; a path that is not a dir → refused (Err); watching failing later (a dir refused, or none of `dirs` left) → watch ends by itself and drops `changed`; guard drop stops and joins. Pure access/open events (taking a lock file) are not changes. Bursts may coalesce.
- Mirror Linux watch.rs structure (one thread; guard drop stops it). Shared, OS-neutral code for mac+win in one module.
- Tests: the Linux watch behaviour tests (crates/carrova-platform/tests/linux/watch/unit.rs) must also run on macOS and Windows CI against the notify impl — e.g. move the behavioural tests to an integration test through the native `Platform` (if the crate exposes one to tests) or a shared test file included by both unit modules. Timeouts generous for FSEvents latency.
- Remove resolved shortcut comments: platform.rs watch_dirs, tray.rs:295. Tray then follows CLI visibility changes on all OSes through the existing watch path; presence keeps POLL only as fallback.
- Verify: host tests; clippy for windows-msvc and aarch64-apple-darwin targets; `cargo deny check` if the justfile has it.
- Commit scope: deps (dependency add) + platform + tauri.

## Task 4: Docs

- docs/PLAN.md: record the follow-ups done (Phase 2 known devices section / Phase 5 notes as fits).
- tasks/todo.md: tick the four follow-ups done (keep the density note + real-hardware item open), add a short results note.
- Confirm no resolved `shortcut:` comments remain (grep).
- Commit scope: docs.
