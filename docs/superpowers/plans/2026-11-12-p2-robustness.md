# Phase 2 follow-ups, round 2: robustness — plan

Context: PR #10 (branch feat/p2-robustness) left documented limits. User (2026-11-12) wants them fixed production-grade. Decisions (user): mDNS = fresh daemon per advert; Nearby overflow = "+N more" chip → List view; capacity grows with field width; long names = ellipsis + native title tooltip.
Spec: /home/ezio/Desktop/Work/personal/porter/docs/superpowers/specs/2026-11-12-background-nearby-design.md. ADR (binding): docs/adr/0001-known-devices.md.

## Global Constraints

- Worktree /home/ezio/Desktop/Work/personal/porter-wt/p2-robustness, branch feat/p2-robustness. Never force-push, reset --hard, git clean. Don't push (controller does).
- CLAUDE.md + tasks/lessons.md bind. Boundary rule 1: no OS-specific code outside carrova-platform; inside it, OS-neutral modules (notify_watch.rs, tool.rs) reach OS code only via `crate::sys`. Rule 2: src-tauri has no business logic.
- unsafe forbidden, no FFI, no new dependencies. Libraries: thiserror, no unwrap/expect/panic outside tests. Never log MACs, names in adverts, secrets.
- Tests outside src/ (`tests/<module>/unit.rs` via #[path], integration `tests/<module>/main.rs`; desktop `apps/desktop/tests/<module>/…`). `just test-layout` enforces. Test first, proven RED.
- Ponytail: smallest complete change, reuse existing helpers, delete what the change makes dead. Resolved `shortcut:` comments removed; new known limits get `shortcut: <limit>, <when to upgrade>`.
- Heavy jobs: `systemd-run --user --scope -p MemoryMax=4G --nice=10 -E CARGO_BUILD_JOBS=3 -E CARGO_TARGET_DIR=/home/ezio/Desktop/Work/personal/porter/target <cmd>`; `touch crates/<crate>/src/lib.rs` before testing an edited crate (shared target dir; it was just `cargo clean`ed so first build is long). Check `cat /proc/loadavg` first; wait if load1>12. Never run full `just ci`.
- Cross-target: `cargo clippy -p carrova-platform --all-targets --target x86_64-pc-windows-msvc|aarch64-apple-darwin -- -D warnings`.
- Commits: Conventional, scopes core|transport|platform|desktop|e2e|docs|…; subject ≤60 chars, imperative; split; NO AI attribution / Co-Authored-By. `just commitlint origin/main HEAD` must pass.

## Task 1: Bounded tool runs (platform)

Files: crates/carrova-platform/src/tool.rs (`stdout_of`), tests/tool/unit.rs; callers macos/mod.rs (scutil, arp), windows/mod.rs (ARP.EXE).
- `stdout_of` gets a deadline (const ~3 s) and an output cap (~64 KiB). Use `crate::sys::isolate_tool(&mut cmd)` before spawn; spawn stdin null, stdout piped, stderr null; read `stdout.take(CAP+1)` to end on a thread → mpsc; `recv_timeout` until deadline; then poll `try_wait` (10 ms) to the same deadline (pattern: crates/carrova-plugin/src/tool.rs:72-120). On timeout / read error / over cap / non-success: `crate::sys::kill_tool(&mut child)` (unix kills process group; windows child.kill), then `child.wait()` (no zombie), return None. Keep `from_utf8_lossy`. Update the "No timeout" doc.
- Tests (run on Linux, tool is compiled under cfg(test)): `sleep 30` with a short limit returns None in < 1 s; `yes` flood hits cap → None; `sh -c 'sleep 30 & wait'` → group killed (`kill -0 <pid>` fails); existing lossy-decode test kept. Make limit/cap parameters of an inner fn so tests use small values. Windows CI: `cmd /c ping -n 20 127.0.0.1` with short limit → None (cfg(windows) test).
- Commit scope platform.

## Task 2: Watch robustness (platform)

Files: crates/carrova-platform/src/notify_watch.rs, src/macos/ (new fn), src/windows/fs.rs (fn beside `fs_id` ~288), lib.rs `crate::sys` wiring, tests (tests/notify_watch/unit.rs, tests/watch/main.rs).
- Dir identity: per-OS `dir_id(path, &Metadata) -> io::Result<DirId>` reached via `crate::sys`: macOS `(dev, ino)` from MetadataExt; Windows `winapi_util::Handle::from_path_any` + `winapi_util::file::information` → `(volume_serial_number, file_index)` (reuse/share with `fs_id`). `present_dirs` returns `(PathBuf, DirId)`; the recheck compares identities too, so a dir deleted+re-created within one RECHECK is re-watched. Identity unavailable → treat as unchanged (today's behaviour). Delete the RECHECK `shortcut:` comment.
- Deadline recheck: `next = Instant::now() + RECHECK`; `recv_timeout(next.saturating_duration_since(now))`; non-change events never postpone the recheck; reset `next` only after a recheck or a reported change.
- Transient re-add errors: a non-NotFound `paths.add`/watch error during rewatch no longer ends the watch at once; retry on the next recheck and end only after N (e.g. 3) consecutive failures (named const + one-line why). Initial watch refusal (a path that is not a dir) still returns Err.
- Tests: pure logic testable on any host — make RECHECK a parameter of `follow` (or the loop core) so tests use ~50 ms; assert recheck still happens under a stream of non-change events; identity change → wake; missing dir drops out; transient failures tolerated up to N. Integration (tests/watch/main.rs, runs on all OSes via `current()`): remove_dir + create_dir inside one RECHECK → `changed` fires, then a write in the new dir fires again.
- Commit scope platform.

## Task 3: mDNS withdraw: fresh daemon per advert (transport, discovery only)

Files: crates/carrova-transport/src/discovery/advertise.rs (`Advertiser::publish`, ~98-107, field docs), src/tuning.rs (MDNS_WITHDRAW_WAIT doc ~116), tests/loopback/main.rs (~254-258 sleep).
Research (mdns-sd 0.21.7, ~/.cargo/registry/src/index.crates.io-*/mdns-sd-0.21.7): unregister (`exec_command_unregister` service_daemon.rs:4164-4216) does not drop `delayed_responses` (queued 10–50 ms answers with the old TXT incl. `dn=`, :3570-3578, :3658-3668, sent at :1717-1726); goodbye resent 120 ms later (:4182,4195) names the reused instance and so drops the NEW advert from peers' caches. `ServiceDaemon::shutdown()` → `cleanup()` sends goodbyes then clears retransmissions + delayed_responses (:1553-1610) before `DaemonStatus::Shutdown` (:1697-1701, :847-853).
- `publish`: if something was registered, take the daemon, `shutdown()`, wait up to MDNS_WITHDRAW_WAIT for `DaemonStatus::Shutdown` (warn on timeout, no names in logs), then create a fresh `ServiceDaemon` and register on it. New daemon creation failure → log, leave `None`, a later publish retries. Field doc: "a fresh daemon per advert: shutdown drops queued answers". No TXT/format/handshake change.
- Regression test first (tests/loopback): a background thread browses in a loop through the switch (keeps PTR queries arriving); pairing → resume ×3; after each `advertise_resume` returns, a fresh browse sees no name. Prove it fails ≥1 in ~20 runs before the fix (capped, load-checked loop); then remove the 250 ms sleep + fix its comment, and show 0 fails in ≥30 runs after.
- Commit scope transport.

## Task 4: Nearby map: slots for any device count (desktop)

Files: apps/desktop/src/screens/nearby/{spots.ts,NearbyMap.tsx,NearbyScreen.tsx,nearby.css,fixtures.ts}, src/ipc/fake/{devices.ts,scenario.ts} (if a bigger scenario is needed), locale files (en + pseudo-locale handling as the repo does), tests/screens/nearby/spots.test.ts (+ a place() test file), e2e/flows/nearby.spec.ts, tasks/todo.md density line.
- Cap pill width: `.s-near__dev` `max-inline-size: 164px`; name/sub lines `overflow:hidden; text-overflow:ellipsis; white-space:nowrap`; native `title` with the full name; accessible name stays full.
- `spots.ts`: replace SWEEP/start/count spacing with `SLOTS[ring] = [{ angle, minField }]` in fill order (alternate right/left), angles clear of card (top-start, 288×200), visibility pill (bottom centre, 298×38), centre (87×90 + label) and field bounds; `minField` = narrowest field width where the slot fits (fields 508 / 888 / 1228 px = windows 720/1100/1440 minus 212 sidebar). Keep `--u`, inner `--k` floor, FLATTEN, RTL mirroring unless the slot geometry needs otherwise. One reserved slot for the "+N more" chip.
- Pure `place(devices, prev, fieldWidth, selected) → { placed: {key, ring, slot}[], more: Device[] }`: sticky (a present device keeps its slot), new device → first free valid slot on its ring, selected device always placed (takes its ring's last-filled slot if full), ring change moves it, narrowing moves too-wide slots to `more`. Overflow priority when full (dropped first): away, then new, then known; stickiness wins.
- NearbyMap: `useRef` Map for sticky slots, one ResizeObserver on `.s-near__field` (reuse an existing hook if any), render placed + chip; new `onMore` prop; NearbyScreen passes `() => on.onView('list')`. Chip: plural i18n "+N more", aria-label "Show all N devices in the list". DOM order inner→outer by slot, chip last. Reduced motion respected if any transition is added.
- Tests: geometry — for each field width, ALL valid slots + chip at once clear each other (6px gap), card, pill, centre and bounds (replaces the split loop; drop "≤2 inner" guarantee text); `place()` 0–30 devices: placed+more = input, placed ≤ valid slots, add/remove one keeps others' slots, selected always placed, ring kept, narrowing only moves too-wide slots, priority. e2e: crowded fixture (~12 devices, one very long name): overlap assertion at full capacity at 720×560/1100×520/1440×900, chip shows +N, clicking it opens List. Existing gallery/a11y/pseudo-locale e2e green.
- Delete the "up to 5 devices / ≤2 inner" comments; close the density item in tasks/todo.md.
- Visual check vs mockup; screenshots /tmp/nb-v2-<scenario>-<WxH>.png.
- Commit scopes desktop / e2e.

## Task 5: Docs

- tasks/todo.md + docs/PLAN.md: record these fixes; remove resolved limits; keep the real-hardware item open.
- grep resolved `shortcut:` comments; none left for these items.
- Commit scope docs.
