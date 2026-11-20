# Background Mode, Nearby Devices and Quiet-Instrument UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Carrova runs quietly in the background with a tray icon, alerts the user (pop-up, screen wake, in-app bell) when a known computer wants to send, shows nearby computers, lets known computers skip the code, and moves every screen to the approved "quiet instrument" design.

**Architecture:** Three build phases, each shippable on its own, then a hardening phase. Phase 1 needs no network or crypto change (UI shell, settings sections, background mode, notifications for existing events). Phase 2 adds device identity, known devices and nearby discovery (crypto + discovery; sign-off gate). Phase 3 adds the code-less request protocol (handshake; sign-off gate). Phase 4 measures, hardens and cleans. Business rules live in `carrova-core`/`carrova-view`; OS calls in `carrova-platform`; `src-tauri` only wires; the React UI renders view-models.

**Tech Stack:** Rust 2024 (workspace crates), Tauri 2.12 (`tray-icon` feature, `tauri-plugin-notification`, `tauri-plugin-autostart` not used: autostart is a platform trait method), React + TypeScript, Vitest, Playwright, ts-rs bindings.

**Spec:** `docs/superpowers/specs/2026-11-12-background-nearby-design.md` · Mockup: `docs/mockups/gui-v4.html`

## Global Constraints

- OS-specific code only in `crates/carrova-platform` (no `cfg(target_os)` elsewhere).
- `apps/desktop/src-tauri` holds no business logic: commands forward to `ops/*`, ops call core/view.
- Any change in `crates/carrova-crypto` or `crates/carrova-transport/src/handshake` needs the user's explicit sign-off (Tasks 2.0 and 3.0 are the gates; nothing after a gate starts before it).
- Never log keys, secrets, pairing codes, device identity keys, known-device secrets or MAC addresses.
- New `Settings` fields carry `#[serde(default)]` so existing `desktop-settings.json` files load.
- Tests outside `src/` (`tests/<module>/unit.rs` via `#[path]`, `tests/<module>/main.rs`); front-end tests in `apps/desktop/tests/<module>`; bug fix → regression test first.
- Copy: sentence case, plain verbs, no all-caps labels, nothing said twice on a screen; strings in `apps/desktop/src/i18n/en/*`.
- Heavy jobs run in the memory-capped unit (`tasks/lessons.md`): `systemd-run --user --unit=carrova-ci ... just ci`.
- Each phase ends with `just ci` green (output shown), `just e2e-ui` green, and a PR the user reviews; commit/push only when the user asks.

## Review Focus

1. **Window destroyed while a flow runs** (user closes mid-transfer with "Keep running" on): the move continues, the tray shows progress state, reopening shows the live flow, not Home. → test in Task 1.6.
2. **No tray host on Linux** (plain GNOME without AppIndicator): closing must minimise, never vanish with no way back. → test in Task 1.6.
3. **Settings file from an older version**: missing new fields load as defaults; unknown future fields don't crash. → test in Task 1.2.
4. **Stranger on the network** while visibility is "Known devices": connection rejected before any notification, rate-limited, nothing logged identifying it. → test in Task 3.2.
5. **"Everyone for 10 minutes" across sleep/restart**: expiry is a wall-clock deadline persisted, so after resume or restart it is already reverted if past. → test in Task 2.4.

---

## Phase 0 · Pre-flight

### Task 0.1: Land the pending Settings layout fix

**Files:** already modified on `main`: `apps/desktop/src/ui/List/List.css`, `apps/desktop/e2e/flows/archive.spec.ts`.

- [ ] **Step 1:** Ask the user to commit it (branch `fix/settings-row-wrap`): `fix(desktop): drop wide choice bars below their title`.
- [ ] **Step 2:** Create the phase branch from updated `main`: `git switch -c feat/quiet-instrument`.

---

## Phase 1 · Quiet-instrument UI, background mode, notifications

Ships without any network or crypto change. Nearby page appears in Phase 2.

### Task 1.1: App shell — header bar, no repeated titles, bell slot

**Files:**
- Modify: `apps/desktop/src/ui/layout/HubLayout.tsx` (add `HeaderBar`; drop `PageHead` title on hub pages), `apps/desktop/src/ui/layout/FlowLayout.tsx` (← back arrow instead of ×), `apps/desktop/src/ui/layout/chrome.ts`, `apps/desktop/src/ui/layout/layouts.css`
- Modify: `apps/desktop/src/app/shell/chrome.ts` (`HUB` labels; Apps label → `nav.apps` = "Apps")
- Modify: `apps/desktop/src/i18n/en/common.ts`, `apps/desktop/src/i18n/en/apps.ts` (title "Apps")
- Test: `apps/desktop/tests/ui/layouts.test.tsx`, `apps/desktop/tests/app/shell/chrome.test.ts`

**Interfaces:**
- Produces: `HeaderBar({ tools?: ReactNode, bell: ReactNode })`; `HubLayout` prop `tools?: ReactNode` (rendered in the header bar); `HubLink` gains `live?: boolean` (green dot).

- [ ] **Step 1: Failing tests** — `flow bar starts with a back arrow labelled Back that returns to the previous view` (FlowLayout, replaces the × close); `hub page renders no h1 equal to its sidebar label` (render each hub route via `renderApp`, assert no heading whose text equals the active `HubLink.label`); `header bar renders tools then bell`; `sidebar Apps label is "Apps"`.
- [ ] **Step 2:** Run `pnpm --filter desktop exec vitest run tests/ui/layouts.test.tsx tests/app/shell` → FAIL.
- [ ] **Step 3:** Implement `HeaderBar` (52 px, tokens only), wire `tools` through `HubLayout`, remove hub `PageHead` titles; keep `PageHead` for flows/sheets.
- [ ] **Step 4:** Re-run → PASS; `just ui-lint`.
- [ ] **Step 5: Commit** `feat(desktop): header bar and single page titles`.

### Task 1.2: Settings contract — new fields

**Files:**
- Modify: `crates/carrova-view/src/ipc/settings.rs`
- Test: `crates/carrova-view/tests/ipc/settings/unit.rs` (or the existing settings test module), `crates/carrova-view/tests/export_bindings/main.rs`

**Interfaces:**
- Produces (all `#[serde(default)]`):
  - `background: BackgroundSettings { keep_running: bool /*true*/, start_at_login: bool /*false*/, lean: bool /*true*/ }`
  - `alerts: AlertSettings { system: bool /*true*/, wake_screen: bool /*true*/, names_on_lock_screen: bool /*false*/, paused_until: Option<u64> /*unix s*/, requests: AlertChannel /*PopupAndBell*/, moves: AlertChannel /*PopupAndBell*/, nearby: AlertChannel /*BellOnly*/, updates: AlertChannel /*BellOnly*/ }`
  - `enum AlertChannel { PopupAndBell, BellOnly }` (snake_case)
  - ts-rs exports for the three new types.

- [ ] **Step 1: Failing tests** — `old_settings_file_loads_with_new_defaults` (parse the current JSON shape without the new keys; assert the defaults above); `unknown_keys_are_ignored`; `alert_channel_round_trips_snake_case`.
- [ ] **Step 2:** `cargo test -p carrova-view settings` → FAIL.
- [ ] **Step 3:** Add the types and defaults; add to `export_all` list; `just bindings`.
- [ ] **Step 4:** `cargo test -p carrova-view && just bindings-check` → PASS.
- [ ] **Step 5: Commit** `feat(view): background and alert settings`.

### Task 1.3: Settings sections with search

**Files:**
- Create: `apps/desktop/src/screens/settings/sections.ts` (registry), `apps/desktop/src/screens/settings/SettingsNav.tsx`, `apps/desktop/src/screens/settings/search.ts`
- Create: `apps/desktop/src/screens/settings/BackgroundGroup.tsx`, `apps/desktop/src/screens/settings/AlertsGroup.tsx`
- Modify: `SettingsScreen.tsx`, `model.ts`, `SettingsGroups.tsx`, `MovesGroup.tsx`, `apps/desktop/src/flows/settings/settingsViews.ts`, `useSettings.ts`, `apps/desktop/src/app/nav/navMachine.ts` (settings section in view), `apps/desktop/src/i18n/en/settings.ts`
- Test: `apps/desktop/tests/screens/settings/settings.test.tsx`, `apps/desktop/tests/screens/settings/search.test.ts`, `apps/desktop/tests/app/nav/navMachine.test.ts`

**Interfaces:**
- Produces: `type SettingsSection = 'general'|'sending'|'nearby'|'notifications'|'moves'|'privacy'|'updates'|'about'`; `SECTIONS: readonly { id: SettingsSection; icon: IconName; rows: readonly SettingRowId[] }[]`; `searchSettings(query: string, t: TFunction): { section: SettingsSection; row: SettingRowId; title: string }[]`; nav view `{kind:'hub', page:'settings', section: SettingsSection}`.
- Consumes: `Settings` fields from Task 1.2 via `useSettings().save(patch)`.

- [ ] **Step 1: Failing tests** — `opening settings swaps the sidebar for sections with a back button`; `each section renders only its rows` (General shows Theme, Reduce motion, Keep running, Start at login, Use as little as possible); `sub-setting dims when parent is off` (Wake the screen disabled when System notifications off); `search "wake" returns the notifications row` (Nearby's WoL row joins in Phase 2); `back returns to the previous hub page`.
- [ ] **Step 2:** Run those tests → FAIL.
- [ ] **Step 3:** Implement; "Nearby & devices" section renders an empty-state line until Phase 2 fills it (hidden from the nav until then: `SECTIONS` filters by a `ready` flag).
- [ ] **Step 4:** PASS; gallery entry `settings/<section>` added in `src/gallery/registry/hub.tsx`.
- [ ] **Step 5: Commit** `feat(desktop): settings sections and search`.

### Task 1.4: Apps master–detail

**Files:**
- Modify: `apps/desktop/src/screens/apps/AppsScreen.tsx`, `model.ts`, `apps.css`; replace `RecipeSheet.tsx` overlay with `AppDetail.tsx` pane; `apps/desktop/src/flows/catalog/catalogViews.ts`; remove `{kind:'recipe'}` from `Overlay` in `navMachine.ts`
- Test: `apps/desktop/tests/screens/apps/apps.test.tsx`, `apps/desktop/tests/flows/catalog/catalogViews.test.ts`

**Interfaces:**
- Produces: `CatalogModel.groups: { category: string; total: number; shown: CatalogEntry[] }[]` (first 5, "Show all"), `CatalogModel.selected: CatalogEntry | null`; `CatalogEntry.here: boolean`; header tools: search + `Segmented` All/Verified/Community.

- [ ] **Step 1: Failing tests** — `rows show only exceptions` (a found app row has no "Here" text; a missing one shows "Not here"); `category count appears once` ("Show all" has no number); `selecting a row fills the detail pane without an overlay`; `left-out items are not struck through`; `search highlights matches`.
- [ ] **Step 2:** FAIL. **Step 3:** Implement per mockup state `apps:*`. **Step 4:** PASS.
- [ ] **Step 5: Commit** `feat(desktop): apps as master-detail`.

### Task 1.5: History re-skin and Send pick at scale

**Files:**
- Modify: `apps/desktop/src/screens/history/HistoryScreen.tsx`, `MoveDetail.tsx`, `moveWords.ts`, `history.css`; `apps/desktop/src/flows/history/historyViews.ts`
- Modify: `apps/desktop/src/screens/send/FoundSelection.tsx`, `SendFoundScreen.tsx`, `model.ts`; `apps/desktop/src/flows/send/sendViews.ts`
- Create: `apps/desktop/src/ui/VirtualList/VirtualList.tsx` (fixed-row virtualiser, no new dependency)
- Test: `apps/desktop/tests/screens/history/history.test.tsx`, `apps/desktop/tests/screens/send/found.test.tsx`, `apps/desktop/tests/ui/virtualList.test.tsx`

**Interfaces:**
- Produces: `moveTag(entry): 'interrupted'|'undone'|'undo_expired'|null` (normal moves → `null`); header tools for History: search + All/Sent/Received/Files; `FoundGroup.collapsed: boolean` (true for every non-key group when total apps > 30), `foundFilter: 'all'|'ticked'|'changed'|'largest'`; `VirtualList<T>({ items, rowHeight, render, overscan = 8 })`.

- [ ] **Step 1: Failing tests** — `history rows carry no tag for a normal finished move`; `history detail shows undo deadline once`; `pick with 340 apps renders keys open and other groups collapsed`; `changed-by-you filter lists only rows differing from the suggestion`; `virtual list renders at most visible+overscan rows for 5000 items`.
- [ ] **Step 2:** FAIL. **Step 3:** Implement; groups open show 8 rows then "Show N more"; lists > 200 rows use `VirtualList`. **Step 4:** PASS.
- [ ] **Step 5: Commit** two commits: `feat(desktop): history shows exceptions only`, `feat(desktop): pick lists that scale to hundreds of apps`.

### Task 1.6: Background mode — tray, close-to-tray, start at login

**Files:**
- Modify: `crates/carrova-platform/src/platform.rs` (new section `// ---- Background ----`), `src/linux/mod.rs` + create `src/linux/background.rs`, `src/macos/background.rs`, `src/windows/background.rs`, `src/fake/background.rs`, `src/types.rs`
- Modify: `Cargo.toml` (`tauri` features `["tray-icon"]`), `apps/desktop/src-tauri/src/lib.rs` (`on_window_event`, `RunEvent`), create `apps/desktop/src-tauri/src/tray.rs`, `apps/desktop/src-tauri/src/ops/background.rs`, `commands/background.rs`
- Create: `crates/carrova-view/src/tray.rs` (menu model)
- Test: `crates/carrova-platform/tests/linux/background/unit.rs`, `crates/carrova-platform/tests/fake/background/main.rs`, `crates/carrova-view/tests/tray/unit.rs`, `apps/desktop/src-tauri/tests/background/unit.rs`

**Interfaces:**
- Produces (platform): `fn has_status_tray(&self) -> bool` (Linux: session bus name `org.kde.StatusNotifierWatcher` owned and `IsStatusNotifierHostRegistered`; macOS/Windows: `true`); `fn autostart_status(&self) -> Result<bool, PlatformError>`; `fn set_autostart(&self, launcher: &Path, on: bool) -> Result<bool, PlatformError>` (Linux `~/.config/autostart/org.byteveda.Carrova.desktop` with `Exec=<launcher> --background`, macOS LaunchAgent plist, Windows `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`). No `%` in launcher on Linux (same rule as file-manager entries).
- Produces (view): `TrayModel { request: Option<TrayRequest>, visible_to: Visibility, nearby_count: Option<u32>, paused: bool }`, `fn tray_menu(model: &TrayModel) -> Vec<TrayItem>` with stable ids `open`, `send`, `nearby`, `pause`, `visibility`, `quit`, `review`, `decline`.
- Produces (desktop): window close → if `background.keep_running` and `has_status_tray()` then destroy webview window and show tray; else if `keep_running` without tray then minimise; else exit. Opening the window removes the tray icon. `--background` launch arg starts with no window.

- [ ] **Step 1: Failing tests** — platform: `autostart_entry_written_and_removed` (fake home), `launcher_with_percent_refused`, `no_watcher_means_no_tray` (fake hook); view: `tray_menu_puts_request_first`, `tray_menu_without_request_starts_with_open`; desktop: `close_with_keep_running_and_tray_hides`, `close_without_tray_minimises`, `close_with_keep_running_off_exits`, `reopen_during_transfer_restores_flow` (Review Focus 1: `liveView` resumes from `sessions()`).
- [ ] **Step 2:** `cargo test -p carrova-platform background && cargo test -p carrova-view tray && cargo test -p carrova-desktop background` → FAIL.
- [ ] **Step 3:** Implement. The tray reflects `TrayModel` only; the window is recreated with the same label `main`; the UI re-fetches state on mount (EngineStateProvider already subscribes on mount; add `port.sessions()` refresh in `Startup`). Settings → General rows call `set_background` (new command) which saves settings and calls `set_autostart`.
- [ ] **Step 4:** PASS, then manual check on this machine: close window → icon appears; open → icon gone; `ps -o rss` of the app with window closed recorded in the PR.
- [ ] **Step 5: Commit** per crate: `feat(platform): autostart and tray host detection`, `feat(view): tray menu model`, `feat(desktop): keep running in the tray`.

### Task 1.7: Notifications — inbox, routing, pop-ups, screen wake, bell

**Files:**
- Create: `crates/carrova-core/src/inbox/mod.rs` (store), `crates/carrova-core/src/inbox/route.rs` (pure routing)
- Modify: `crates/carrova-platform/src/platform.rs` (`fn wake_display(&self) -> Result<(), PlatformError>`), linux/macos/windows/fake impls
- Modify: `Cargo.toml` + `apps/desktop/src-tauri/Cargo.toml` (`tauri-plugin-notification`), `capabilities/default.json` (`notification:default`), `src-tauri/src/events/sink.rs` (feed finished/failed events to the inbox), create `src-tauri/src/ops/inbox.rs`, `commands/inbox.rs`
- Modify: `crates/carrova-view/src/ipc/mod.rs` (`BridgeEvent::Notice { notice: Notice }`), create `crates/carrova-view/src/ipc/inbox.rs`
- Front-end: create `apps/desktop/src/ui/Bell/Bell.tsx`, `apps/desktop/src/flows/inbox/useInbox.ts`, `inboxViews.ts`; modify `src/ipc/port.ts`, `src/ipc/tauri.ts`, `src/ipc/fake/*`, `src/app/state/appState.ts`, `src/i18n/en/common.ts`
- Test: `crates/carrova-core/tests/inbox/main.rs`, `crates/carrova-core/tests/inbox/route/unit.rs`, `crates/carrova-platform/tests/fake/wake/main.rs`, `apps/desktop/tests/ui/bell.test.tsx`, `apps/desktop/tests/flows/inbox/inboxViews.test.ts`, `apps/desktop/e2e/flows/bell.spec.ts`

**Interfaces:**
- Produces (core): `Notice { id: NoticeId, kind: NoticeKind, at: u64, read: bool, peer: Option<String>, summary: String }`; `NoticeKind { SendRequest, MoveFinished, MoveFailed, DeviceNearby, UpdateReady }`; `Inbox::open(platform) -> Result<Inbox>`, `push(Notice)`, `list() -> Vec<Notice>` (newest first, entries older than 30 days dropped on load), `mark_read(ids)`, `mark_all_read()`; file `state_dir/inbox.json`, 0600, atomic write.
- Produces (route): `fn route(kind: NoticeKind, prefs: &AlertSettings, now: u64, screen_locked: bool) -> Delivery` where `Delivery { popup: bool, wake: bool, show_names: bool }`; rules: paused → popup=false; `BellOnly` → popup=false; wake only for `SendRequest` and only if `wake_screen`; `show_names = !screen_locked || names_on_lock_screen`.
- Produces (port): `notices(): Promise<Notice[]>`, `markNoticesRead(ids: NoticeId[] | 'all')`; event `notice`.
- UI: `Bell` button in `HeaderBar`, popover over current page; closes on outside click, Escape, second click, navigation; Review in a request notice → `enterFlow receive`.

- [ ] **Step 1: Failing tests** — `inbox_drops_entries_older_than_30_days`; `inbox_file_is_0600`; `paused_alerts_never_pop_up`; `bell_only_never_pops_up`; `wake_only_for_send_requests`; `locked_screen_hides_names_unless_allowed`; UI: `bell opens over the current page without navigating`, `outside click closes the bell`; e2e: open bell on Settings → URL/view unchanged.
- [ ] **Step 2:** FAIL. **Step 3:** Implement; `wake_display`: Linux D-Bus `org.freedesktop.ScreenSaver.SimulateUserActivity` via `zbus` (add as a platform dependency, `default-features=false, features=["blocking"]`), macOS `IOPMAssertionDeclareUserActivity`, Windows `SetThreadExecutionState(ES_DISPLAY_REQUIRED|ES_SYSTEM_REQUIRED)`; all best-effort (errors logged at debug, never surfaced). **Step 4:** PASS.
- [ ] **Step 5: Commit** per crate: `feat(core): notification inbox and routing`, `feat(platform): wake the display`, `feat(desktop): pop-ups and the bell`.

### Task 1.8: Phase 1 gate

- [ ] `just ci` in the capped unit → `CI EXIT 0` (show counts).
- [ ] `just e2e-ui` all green; gallery updated.
- [ ] `boundary-reviewer` agent on the phase diff; fix findings.
- [ ] Open PR `feat/quiet-instrument` when the user asks.

---

## Phase 2 · Device identity, known devices, Nearby

### Task 2.0: Sign-off gate — identity ADR

**Files:** Create `docs/adr/00NN-known-devices.md` (next free number).

- [ ] **Step 1:** Write the ADR: per-install Ed25519 identity (`DeviceKey`), fingerprint format `XXXX·XXXX` (first 4 bytes of SHA-256 of the public key, hex upper), exchange inside the PAKE-confirmed channel (each side signs `ChannelBinding` bytes + its public key), per-pair secret `KnownSecret` = HKDF(SessionKeys, label `carrova/1/known`) stored per known device, discovery tag = HMAC(KnownSecret, purpose `known-discovery` ‖ nonce), threat model (stranger, spoofed name, stolen state dir, replay), and what is never logged.
- [ ] **Step 2:** Present to the user. **Stop until the user signs off in writing.** Record the date in the ADR.

### Task 2.1: Device identity and known-device store

**Files:**
- Create: `crates/carrova-crypto/src/identity.rs` (`DeviceKey`, `DevicePublic`, `Fingerprint`), `crates/carrova-crypto/src/pairing/known.rs` (`KnownSecret`, `KnownPurpose`)
- Create: `crates/carrova-core/src/devices/mod.rs` (`KnownDevices` store), `crates/carrova-core/src/devices/record.rs`
- Test: `crates/carrova-crypto/tests/identity/unit.rs`, `crates/carrova-crypto/tests/pairing/known/unit.rs`, `crates/carrova-core/tests/devices/main.rs`

**Interfaces:**
- Produces (crypto): `DeviceKey::generate()`, `DeviceKey::from_bytes(&[u8;32])`, `public() -> DevicePublic`, `sign_binding(&ChannelBinding) -> [u8;64]`; `DevicePublic::verify_binding(&ChannelBinding, &[u8;64]) -> Result<()>`; `Fingerprint::of(&DevicePublic) -> Fingerprint` (Display `7F3A·19C2`); `SessionKeys::known_secret() -> KnownSecret`; `KnownSecret::tag_for(KnownPurpose, &TagNonce) -> Tag`, `recognises(...) -> bool`. `Debug` impls redact secrets.
- Produces (core): `DeviceRecord { fingerprint: Fingerprint, public: DevicePublic, name: String, os: OsKind, first_seen: u64, last_move: Option<u64>, moves: u32, mac_hint: Option<[u8;6]> }`; `KnownDevices::open(platform)`, `remember(record, secret)`, `forget(&Fingerprint)`, `forget_all()`, `rename(&Fingerprint, name)`, `list()`, `secret_for(&Fingerprint) -> Option<KnownSecret>`; files `state_dir/devices/known.json` + `state_dir/devices/<fp>.key` (0600, owner-checked, like `store/resume_key.rs`); own key `state_dir/device.key`.

- [ ] **Step 1: Failing tests** — `signature_over_other_binding_fails`; `fingerprint_display_format`; `known_secret_differs_from_resume_secret`; `debug_output_has_no_key_bytes`; store: `remember_then_list`, `forget_removes_key_file`, `files_are_0600`, `rename_keeps_identity`, `corrupt_store_is_reported_not_overwritten`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(crypto): device identity and known-device secret`, `feat(core): known devices store`.

### Task 2.2: Remember devices after a code-paired move

**Files:**
- Modify: `crates/carrova-transport/src/handshake/{sender.rs,receiver.rs,keys.rs}`, `crates/carrova-transport/src/wire/messages.rs` (new `Identify { public: [u8;32], sig: [u8;64] }` after `Established`), `crates/carrova-core/src/engine/send/pairing.rs`, `crates/carrova-core/src/engine/receive.rs`, `crates/carrova-view/src/ipc/settings.rs` (`nearby: NearbySettings { remember: bool /*true*/, accept_known: bool /*true*/, wol_hints: bool /*false*/ }`)
- Test: `crates/carrova-transport/tests/handshake/main.rs`, `crates/carrova-core/tests/devices/remember/main.rs`

**Interfaces:**
- Produces: `Authenticated.peer_identity: Option<DevicePublic>`; old peers that send no `Identify` still pair (field `None`) — version-tolerant.
- [ ] **Step 1: Failing tests** — `paired_move_remembers_both_sides` (two engines in one process); `remember_off_stores_nothing`; `identify_with_bad_signature_fails_session`; `old_peer_without_identify_still_pairs`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(transport): exchange device identity after pairing`, `feat(core): remember paired devices`.

### Task 2.3: Visibility and presence discovery

**Files:**
- Modify: `crates/carrova-transport/src/discovery/{advertise.rs,browse.rs,seek.rs}`, `crates/carrova-transport/src/lib.rs` (TXT `kt` known tag, `os` OS kind), `crates/carrova-core/src/lan/nearby.rs`
- Create: `crates/carrova-core/src/presence.rs` (`Presence` service: advertises according to visibility)
- Test: `crates/carrova-transport/tests/discovery/presence/main.rs`, `crates/carrova-core/tests/presence/main.rs`

**Interfaces:**
- Produces: `enum Visibility { NoOne, Known, Everyone { until: u64 } }` (in `carrova-view` settings, `#[serde(default)]` → `Known`); `Presence::start(engine, Visibility) -> Presence`, `set(Visibility)`, `stop()`; `browse_nearby(platform) -> Vec<NearbyDevice>` with `NearbyDevice { name: String, os: Option<OsKind>, known: Option<Fingerprint>, answered_ms: u32 }`.
- Rules: `NoOne` → no advert; `Known` → advert carries one `kt` per known device (fresh nonce each), no `dn`; `Everyone` → also `dn` and `os`, until `until` (wall clock).
- [ ] **Step 1: Failing tests** — `known_mode_hides_name_from_strangers`; `known_peer_recognises_its_tag`; `tags_from_two_adverts_are_unlinkable`; `everyone_reverts_after_deadline` and `everyone_already_past_deadline_on_start_is_known` (Review Focus 5); `no_one_advertises_nothing`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(transport): presence adverts for known devices`, `feat(core): visibility modes`.

### Task 2.4: Nearby page, known devices settings, CLI

**Files:**
- Front-end: `apps/desktop/src/app/nav/navMachine.ts` (`HubPage` adds `'nearby'`), create `apps/desktop/src/screens/nearby/{NearbyScreen.tsx,NearbyMap.tsx,NearbyList.tsx,VisibilityPill.tsx,DeviceCard.tsx,model.ts,nearby.css}`, `apps/desktop/src/flows/nearby/{useNearby.ts,nearbyViews.ts}` (move from `flows/receive/useNearby.ts`), `apps/desktop/src/screens/settings/NearbyGroup.tsx`, `KnownDevicesSheet.tsx`, `apps/desktop/src/i18n/en/nearby.ts`
- Desktop bridge: `apps/desktop/src-tauri/src/{ops,commands}/devices.rs`
- CLI: `apps/cli/src/devices.rs` (`carrova devices list|rename|forget`, `carrova visibility no-one|known|everyone`)
- Test: `apps/desktop/tests/screens/nearby/nearby.test.tsx`, `apps/desktop/tests/flows/nearby/nearbyViews.test.ts`, `apps/desktop/e2e/flows/nearby.spec.ts`, `apps/cli/tests/devices/main.rs`

**Interfaces:**
- Consumes: `browse_nearby`, `KnownDevices`, `Visibility` (Tasks 2.1–2.3).
- Produces (port): `nearby(): Promise<NearbyDevice[]>`, `knownDevices()`, `renameDevice(fp, name)`, `forgetDevice(fp | 'all')`, `setVisibility(v)`.
- UI rules: visibility pill is the only visibility status (sidebar shows a dot only); map ring = `answered_ms` bucket (< 300 → inner); the scan pulse plays once per page open, never under reduced motion; known → "Send" (Phase 3; until then "Send with code"), new → "Send with code", away → "Forget".
- [ ] **Step 1: Failing tests** — `visibility shows once on the page`; `sidebar shows a dot, not text`; `pill and settings bind one value` (change in one, see in the other); `away devices are faded and offer Forget`; `no pulse under reduced motion`; CLI `devices_list_shows_fingerprints`, `visibility_everyone_sets_deadline`.
- [ ] **Step 2–4:** FAIL → implement (fake port scenario `nearby`) → PASS.
- [ ] **Step 5: Commit** `feat(desktop): nearby page`, `feat(desktop): known devices settings`, `feat(cli): devices and visibility commands`.

### Task 2.5 (optional): Wake-on-LAN hints

**Files:** `crates/carrova-platform` (`fn neighbour_mac(&self, addr: IpAddr) -> Option<[u8;6]>`, `fn send_magic_packet(&self, mac: [u8;6]) -> Result<(), PlatformError>`), `crates/carrova-core/src/devices/wake.rs`, Nearby "Wake" action on away devices with a hint.
- [ ] Tests: `magic_packet_layout` (6×0xFF + 16×MAC), `hint_saved_only_when_setting_on`, `mac_never_logged` (tracing capture). Implement only if the user wants it in this phase.

### Task 2.6: Phase 2 gate — as Task 1.8.

---

## Phase 3 · Code-less send between known devices

> **Superseded.** This phase is replaced by
> `docs/superpowers/specs/2026-11-20-known-device-send-design.md` and
> `docs/adr/0004-known-device-requests.md` (Review hands over a one-time
> code and runs today's code-paired, pinned move). Kept as history only.

### Task 3.0: Sign-off gate — request protocol ADR

**Files:** Create `docs/adr/00NN-known-device-requests.md`.
- [ ] **Step 1:** Write: receiver listens only when visibility ≠ NoOne; new `Hello` intent `Request { from: Fingerprint, nonce, proof }` where proof = HMAC(KnownSecret, binding ‖ nonce) plus identity signature over the binding; unknown fingerprint → close without reply (no notification); admission: max 3 pending requests, 5 attempts/min per source address, 10 s handshake deadline; replay protection via `ResumeNonces`-style first-use set; request summary message `SendRequest { apps: u32, bytes: u64, has_sensitive: bool }` (no names until accepted); decline/timeout (2 min) semantics.
- [ ] **Step 2:** Present. **Stop until the user signs off in writing.**

### Task 3.1: Transport — request intent and receiver listener

**Files:** `crates/carrova-transport/src/handshake/{receiver.rs,sender.rs,secret.rs}`, `wire/messages.rs`, create `crates/carrova-transport/src/admission.rs`
- Test: `crates/carrova-transport/tests/handshake/request/main.rs`, `crates/carrova-transport/tests/admission/unit.rs`

**Interfaces:**
- Produces: `RequestListener::bind(opts) -> RequestListener`, `next(&self, known: &dyn KnownLookup) -> Result<IncomingRequest>`; `IncomingRequest { from: Fingerprint, summary: SendRequest, accept(self) -> Authenticated, decline(self) }`; `connect(Target::Known { fingerprint, secret, key }, ...)`; `trait KnownLookup { fn secret(&self, fp: &Fingerprint) -> Option<KnownSecret>; fn public(&self, fp: &Fingerprint) -> Option<DevicePublic>; }`.
- [ ] **Step 1: Failing tests** — `known_request_is_admitted`; `unknown_fingerprint_closed_without_reply`; `replayed_request_rejected`; `sixth_attempt_in_a_minute_rejected` (Review Focus 4); `wrong_identity_signature_rejected`; `decline_closes_both_sides_cleanly`; wire fuzz `request_messages_never_panic` (proptest, 10k cases).
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(transport): known-device send requests`.

### Task 3.2: Core sessions from requests

**Files:** `crates/carrova-core/src/engine/send/request.rs` (sender asks), `crates/carrova-core/src/engine/receive_request.rs` (receiver accepts → `Importer` with `Source::Lan`), `crates/carrova-core/src/lan/messages.rs`
- Test: `crates/carrova-core/tests/request/main.rs` (two engines in one process)

**Interfaces:**
- Produces: `Exporter::request_send(&mut self, to: &Fingerprint) -> Result<SendHandle>` (reuses `SendHandle::run`); `Engine::listen_requests(cancel) -> RequestInbox`; `RequestInbox::next() -> Result<PendingRequest>`; `PendingRequest::review(self) -> Result<Importer>` / `decline(self)`; receiving then follows today's `plan_for_review`/approve path unchanged.
- [ ] **Step 1: Failing tests** — `request_then_review_moves_files` (end to end, two engines); `decline_leaves_receiver_untouched`; `request_times_out_after_two_minutes` (fake clock); `nothing_written_before_approval`; `sensitive_items_still_asked_after_request`.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(core): send to a known device without a code`.

### Task 3.3: Desktop — requests reach the user

**Files:** `apps/desktop/src-tauri/src/ops/requests.rs`, `commands/requests.rs`, `src/tray.rs` (request item), `src/events/sink.rs` (inbox notice `SendRequest` + `route` + `wake_display`); `crates/carrova-view/src/ipc/mod.rs` (`BridgeEvent::SendRequestArrived { request: RequestView }`)
- Front-end: `apps/desktop/src/screens/receive/{ReadyScreen.tsx,RequestScreen.tsx}`, `apps/desktop/src/screens/send/ChooseComputerScreen.tsx`, `WaitingScreen.tsx`; `apps/desktop/src/app/nav/flows.ts` (send steps Pick → Choose computer → Their review → Move; receive steps Request → Review → Move → Done); `useSendFlow.ts`, `useReceiveFlow.ts`; i18n
- Test: `apps/desktop/src-tauri/tests/requests/unit.rs`, `apps/desktop/tests/flows/send/chooseComputer.test.tsx`, `apps/desktop/tests/flows/receive/request.test.tsx`, `apps/desktop/e2e/flows/known-send.spec.ts`

**Interfaces:**
- Produces (port): `requestSend(session, to: Fingerprint)`, `answerRequest(id, 'review' | 'decline')`; event `send_request_arrived`.
- [ ] **Step 1: Failing tests** — `request while window closed creates notice, tray item and wake (fake platform hooks)`; `locked screen notice has no names`; `choose computer lists known first, marks new "Needs a code once"`; `known device → waiting screen; new → code screen`; `review opens today's review screen`; e2e known-send happy path on the fake port.
- [ ] **Step 2–4:** FAIL → implement → PASS.
- [ ] **Step 5: Commit** `feat(desktop): incoming requests`, `feat(desktop): choose a computer when sending`, `feat(desktop): ready to receive`.

### Task 3.4: Phase 3 gate — as Task 1.8, plus `just e2e --lan` with two containers exercising a code-less send.

---

## Phase 4 · Performance, robustness, production review

Goal: measured budgets, failure handling proven by tests, and a codebase a new contributor can read. Findings become small tasks (regression test first) on `chore/hardening`.

### Task 4.1: Budgets and measurement

**Files:** Create `docs/perf/budgets.md`, `scripts/perf/idle.sh` (wired as `just perf-idle`), `apps/desktop/e2e/perf/lists.spec.ts`
- Budgets (fail the check if exceeded): window closed, idle 5 min → RSS ≤ 60 MB total for Carrova processes, CPU ≤ 0.5 % average; presence adverts ≤ 1 per 5 s; window open to interactive ≤ 800 ms (warm); Send pick with 5 000 apps and History with 10 000 moves → scroll frames ≤ 16 ms p95, filter ≤ 50 ms; settings save ≤ 20 ms.
- [ ] Measure on this laptop (Linux) and record numbers in `budgets.md`; fix any miss before continuing (profile with `perf`/Chrome tracing; no speculative optimisation).

### Task 4.2: Robustness matrix

- [ ] Tests for each, owned by the crate that handles it: network interface change mid-scan (presence restarts), sleep/resume with a pending request (request expires, notice says so), tray host appears/disappears at runtime (re-check on close), notification permission denied (bell still works, Settings explains), corrupt `inbox.json` / `known.json` (moved aside, user told, never silently reset), disk full on inbox write (no crash), clock jump backwards (visibility deadline and 30-day prune behave), two Carrova instances (single-instance holds), peer on an older protocol version (pairs with code, no known-device features).
- [ ] Fuzz: extend wire fuzz to `Identify`, `Request`, `SendRequest`; run 1 M cases in CI nightly (`just fuzz-wire`).

### Task 4.3: Security review

- [ ] Fresh reviewer agent (no prior context) reviews the known-device and request design against both ADRs and the code: stranger admission, replay, spoofed names, fingerprint collisions, state-dir theft, log redaction (grep test: no key/secret/MAC types implement `Display`; tracing capture test across a full request flow finds no fingerprint secret material).
- [ ] Findings fixed with regression tests; residual risks written into the ADRs.

### Task 4.4: Code quality and maintainability

- [ ] `boundary-reviewer` agent over the whole feature diff (no OS code outside platform, no logic in src-tauri, dependencies point down).
- [ ] Module size: no new Rust file over 400 lines, no React component over 250 lines; split where exceeded (`useSendFlow.ts` already ~390 lines → split selection queue/run logic).
- [ ] Remove dead code left by the redesign (old `RecipeSheet` overlay, old `PageHead` uses, `flows/receive/useNearby.ts` shim); `cargo +stable clippy -D warnings` and `-W clippy::pedantic` on new modules, fix or justify each allow.
- [ ] Docs: rustdoc on every new public item (`cargo doc -D warnings` already in lint), update `docs/PLAN.md` status, `CLAUDE.md` status line, `packaging/` notes (autostart file, tray dependency `libayatana-appindicator3-1` in `.deb`/`.rpm` recommends).
- [ ] Accessibility: axe check in `e2e/flows/*` for Nearby, bell popover, settings sections; keyboard path through tray-equivalent actions inside the app; reduced-motion snapshot of the map.
- [ ] i18n: `tests/i18n` key completeness, pseudo-locale run for the new screens (no clipped strings at 130 % length).

### Task 4.5: Hardware sign-off

- [ ] Manual checklist (user): Ubuntu GNOME (tray via AppIndicator), plain GNOME without tray (minimise fallback), KDE, Cinnamon; macOS (menu bar, lock-screen notification, LaunchAgent); Windows 11 (tray, toast, Run key); two real machines: code pairing, remember, code-less send, decline, Everyone-10-min expiry, sleep/wake. Results recorded in `tasks/todo.md`.
