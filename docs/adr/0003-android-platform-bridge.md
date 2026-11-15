# ADR 0003: Android platform bridge

- Status: Accepted
- Signed off: 2026-11-15 by the user (open questions accepted; if `mobile_entry_point` trips `forbid`, src-tauri may use `deny` with one justified `#[allow(unsafe_code)]` on the mobile entry wrapper only)
- Date: 2026-11-15
- Scope: Ecosystem Phase 1, "Android shell"
  (`docs/superpowers/specs/2026-11-15-ecosystem-phase1-design.md`), plan
  Tasks 12, 13 and 21 (`docs/superpowers/plans/2026-11-15-ecosystem-phase1.md`).
- Touches: boundary rule 1 (one sanctioned exception in `src-tauri`) and
  the workspace `unsafe_code = "forbid"` lint (one allow-listed module).
  Both need the user's sign-off.

## Context

Phase 1 builds the existing Tauri app for Android. Clipboard, foreground
service, multicast lock, ring, Downloads, QR and opening URLs/files need
Android Java APIs, which Rust reaches only through JNI.

What exists:

- `carrova-platform` is the only OS-aware crate. Adapters are selected by
  `cfg(target_os)` in `lib.rs` (`linux`, `macos`, `windows` modules,
  `crates/carrova-platform/src/lib.rs:27-36`), and `current()` returns the
  adapter for the build target with one arm per OS
  (`lib.rs:53-68`). There is no Android arm.
- `OsKind` = `Linux | MacOs | Windows`, serde `snake_case`, TS bindings,
  `ALL`, `as_str`, `display_name`, `permission_model`
  (`crates/carrova-platform/src/os.rs:7-54`).
- Workspace lints: `unsafe_code = "forbid"`, `unused_must_use = "deny"`
  (`Cargo.toml:103-106`). `carrova-platform` and `src-tauri` both use
  `[lints] workspace = true` (`crates/carrova-platform/Cargo.toml:52-53`,
  `apps/desktop/src-tauri/Cargo.toml:49-50`).
- The lock file already has `jni` 0.21.1 (used by `tauri`,
  `tauri-runtime`, `tauri-runtime-wry`, `tao`, `wry`), `jni` 0.22.4 (via
  `rustls-platform-verifier`) and `ndk-context` 0.1.1 (via `tao`)
  (`Cargo.lock:2764`, `Cargo.lock:2780`, `Cargo.lock:3194`).
- `ndk-context` is initialised by tao in the **activity's** `onCreate`
  with the JavaVM and the application context
  (`tao-0.37.2/src/platform_impl/android/ndk_glue.rs:389-416`). Without an
  activity it is not initialised.
- `src-tauri`'s entry is `pub fn run() -> tauri::Result<()>`, which builds
  the app and runs `setup` (`apps/desktop/src-tauri/src/lib.rs:74-87`).
  `[lib]` has no `crate-type` (`apps/desktop/src-tauri/Cargo.toml:12-13`);
  there is no `gen/android` project yet (`apps/desktop/src-tauri/gen/`
  holds only `schemas`).
- Tauri's `mobile_entry_point` (tauri-macros 2.7.4,
  `src/mobile.rs:30-110`) wraps the function in `_start_app`, calls it
  through `stop_unwind`, and on Android expands `tauri::android_binding!`,
  which expands tao's and wry's `android_fn!` into `#[no_mangle] unsafe
  extern "C" fn Java_…` functions (`tao-macros-0.1.4/src/lib.rs:248-249`).

## Decision

### 1. OS kind and adapter

- `OsKind::Android` (serde `"android"`, `as_str` `android`, display
  `Android`, permission model `Unix`), added to `ALL`.
- `crates/carrova-platform/src/android/`, compiled under
  `#[cfg(target_os = "android")]`; `current()` gains the Android arm
  returning `AndroidPlatform`. Methods Android cannot do return
  `Unsupported`, as the other adapters do.

### 2. Rust → Kotlin calls

- Through the `jni` crate (0.21, the version Tauri, tao and wry already
  use, so no second copy is added for Android), using the JavaVM and
  application context from `ndk-context`. Both go in
  `[target.'cfg(target_os = "android")'.dependencies]` of
  `carrova-platform` only.
- This needs `unsafe` (`JavaVM::from_raw`, `JObject::from_raw` on the
  context pointer). Allow-list exactly
  `crates/carrova-platform/src/android/jni.rs`:
  - the workspace lint stays `unsafe_code = "forbid"`;
  - `carrova-platform` sets `unsafe_code = "deny"` in its own `[lints]`
    (`forbid` cannot be lifted by an inner `allow`), keeping the other
    workspace lints;
  - `android/jni.rs` alone has `#![allow(unsafe_code)]`, and every
    `unsafe` block carries a `// SAFETY:` comment.
  - Every other module of every crate still rejects `unsafe` (deny in
    platform, forbid elsewhere).
- `jni.rs` exposes safe functions only (attach the current thread, call one
  static Kotlin method, convert arguments and results, map Java exceptions
  to `PlatformError`). Other Android modules call those.

### 3. Kotlin glue

- Lives in
  `apps/desktop/src-tauri/gen/android/app/src/main/java/org/byteveda/carrova/platform/`
  (generated project, committed), app id `org.byteveda.Carrova`.
- One static method per platform call. No decisions, no state beyond what
  the Android API needs (e.g. the held multicast lock, the playing ring).
  Whether to send, accept, ask or retry is decided in Rust.

### 4. Process lifetime

- Kotlin `LinkService`, a foreground service of type `connectedDevice`,
  keeps the process alive while any link exists. It starts at the first
  link and on boot if any link exists.
- The Rust supervisor is started from Tauri's `setup` when the app runs.
- After boot (no activity), two candidate mechanisms; **the Task 13 spike
  decides and records which in this ADR's Consequences** (commit
  `docs(adr): record Android lifetime spike`):
  1. **Tauri entry without the activity:** `LinkService` starts Tauri's
     activity-less entry, which runs `setup` and the supervisor.
  2. **Fallback:** `LinkService` calls `System.loadLibrary` and one
     exported `start_supervisor` through Tauri's mobile entry, which
     starts only the supervisor.
- Either way the supervisor needs a JavaVM and context before any JNI
  call. Tao only initialises `ndk-context` in the activity's `onCreate`
  (Context above), so mechanism 2 must initialise it itself (from the
  service's `JNIEnv` and application context), inside `jni.rs`. The spike
  checks this as well as survival with the activity destroyed.
- If neither mechanism works, the spike stops and asks the user.

### 5. QR scanning

The official `tauri-plugin-barcode-scanner` (Tauri plugins workspace,
mobile only). No own camera code.

### 6. One exception to boundary rule 1

`#[cfg_attr(mobile, tauri::mobile_entry_point)]` on `run()` in
`apps/desktop/src-tauri/src/lib.rs`. It is Tauri's own attribute and
`mobile` is Tauri's own cfg (set by `tauri-build`). No other platform
condition goes in `src-tauri`; everything Android-specific stays in
`carrova-platform` or the Kotlin glue.

**Amended 2026-11-15 (user sign-off):** `src-tauri` may also use Tauri's
own `desktop` / `mobile` cfgs to gate desktop-only or mobile-only plugins
and window chrome (tray, single-instance, window events). They never gate
business logic. `cfg(target_os)` stays forbidden in `src-tauri`. The
mobile entry attribute sits on a small `mobile_main()` wrapper around
`run()` (the macro drops `run()`'s `Result`), the one item with
`#[allow(unsafe_code)]`; `src-tauri` sets `unsafe_code = "deny"`.

## Alternatives considered

| Option | Why not |
|---|---|
| Native Kotlin app + UniFFI | Rejected in the parent design: a second UI and contract to keep in sync. |
| Tauri mobile plugins (Kotlin plugin per feature, called from JS/Rust via the plugin API) | Puts Android calls in `src-tauri` plugin code, outside `carrova-platform`, and makes the CLI/headless supervisor depend on the Tauri runtime. |
| `unsafe` allowed crate-wide in `carrova-platform` | Wider than needed; one file is enough. |
| Separate crate just for the JNI bridge | Would be a second OS-aware crate (boundary rule 1). |
| `jni` 0.22 | Newer, but Tauri/tao/wry pin 0.21; two versions on Android for no gain. Revisit when Tauri moves. |
| Own camera/QR code | The official plugin exists and is maintained with Tauri. |

## Consequences

- `OsKind` gains a variant: TS bindings (`OsKind.ts`) and the front-end
  `osName` helper change; manifests may now carry `android` as source OS.
- `carrova-platform/Cargo.toml` gets its own `[lints]` table (it can no
  longer use `workspace = true` alone, since it changes one lint); the
  other workspace lints must be repeated there and kept in sync.
- The Tauri lib needs `crate-type = ["staticlib", "cdylib", "rlib"]` for
  the Android build (Task 12/21).
- **Risk, to verify in Task 12/21:** the `mobile_entry_point` expansion
  contains `#[no_mangle] unsafe extern "C"` JNI functions (via
  `android_binding!`) inside `src-tauri`, which the workspace
  `unsafe_code = "forbid"` may reject on the Android build. If it does,
  this needs another sign-off (e.g. `src-tauri` at `deny` with one
  `#[allow(unsafe_code)]` on `run()` and a comment naming the macro).
- **Risk, to verify in Task 12/21:** the macro calls `run()` and drops its
  return value; `run()` returns `tauri::Result<()>` and the workspace
  denies `unused_must_use`. If it fires, the attribute goes on a
  `run()`-calling wrapper that logs the error, still the one exception.
- `connectedDevice` services need `FOREGROUND_SERVICE_CONNECTED_DEVICE`
  plus one prerequisite permission on Android 14+;
  `CHANGE_WIFI_MULTICAST_STATE` (needed for the multicast lock anyway) is
  expected to serve; the spike confirms, including start from boot.
- **Lifetime spike result (Task 13, 2026-11-15, emulator API 34 x86_64,
  debug APK with throwaway spike code): mechanism 2.**
  - **Mechanism 1 fails.** Tauri has no activity-less entry: its only
    Android entry is `Rust.onFirstActivityCreate()` (wry's
    `WryActivity`/`WryLifecycleObserver`). Called from `LinkService`
    without an activity, it returns but `setup` never runs (no supervisor
    for 25 s); when the activity opens later, the lifecycle observer calls
    it again and `setup` runs **twice** in one process (two Tauri apps).
  - **Mechanism 2 works.** `LinkService` calls `System.loadLibrary` and
    one exported `startSupervisor(context)`; the supervisor thread runs
    with no activity and its JNI calls work (heartbeats every 10 s, same
    pid). Opening the activity afterwards starts Tauri normally (UI
    renders, no crash). After `adb reboot`, a `BOOT_COMPLETED` receiver
    starting `LinkService` (`connectedDevice`, prerequisite permission
    `CHANGE_WIFI_MULTICAST_STATE`) brought the supervisor up with no
    activity on API 34.
  - **Not `ndk_context::initialize_android_context`** (amends Decision
    §4): it asserts it was unset, and tao calls it in the activity's
    `onCreate`, so initialising it from the service would abort the
    process when the user opens the app. Instead `android/jni.rs` keeps
    the service's JavaVM and context itself
    (`carrova_platform::attach_service_context`) and falls back to
    ndk-context (set by tao) otherwise.
  - **Activity destroyed: Tauri exits the process unless told not to.**
    With the stock `run()`, removing the task ends tao's event loop,
    which calls `std::process::exit` and aborts the process (SIGABRT in
    EGL teardown) despite the running foreground service; Android
    restarts the sticky service in a new process. With
    `RunEvent::ExitRequested` → `api.prevent_exit()` on mobile, the
    supervisor thread started in `setup` kept running for 100 s+ (same
    pid) after the activity was destroyed. Reopening the app then shows
    a blank screen (Tauri dropped the window): Task 21 must re-create the
    main window when a new activity arrives.
  - **For Task 21:** the exported `startSupervisor` must live in
    `src-tauri` (it starts core's supervisor, which `carrova-platform`
    cannot reach), so it is a second `#[unsafe(no_mangle)]` item there
    with `#[allow(unsafe_code)]`: needs the user's sign-off then.
    Supervisor start must be once per process (service and `setup` both
    try).

## Open questions for the user

1. Accept `carrova-platform` moving from `forbid` to `deny` (with only
   `android/jni.rs` allowed)?
2. If the Tauri macro trips `forbid(unsafe_code)` in `src-tauri` on
   Android, is one more scoped `allow` on `run()` acceptable (new sign-off
   at that time)?
3. Commit the generated `gen/android` project (needed for the Kotlin glue
   to live there), or generate it in CI and keep only the glue in a
   separate directory?

Signed off: 2026-11-15 by the user. Open questions accepted as proposed, including the scoped src-tauri allowance for the mobile entry point if needed.
