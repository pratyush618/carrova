# Manual mode and "Send with Carrova" — Implementation Plan

> For agentic workers: implement task-by-task; steps use `- [ ]` checkboxes. Every implementation brief also invokes `ponytail:ponytail` (smallest complete change) and the project quality bar (tasks/lessons.md).

**Goal:** Manual mode: the user picks exact files/folders (system picker, file-manager right-click on Linux/Windows/macOS, or `carrova send|export PATH...`), and those picks go through the same capture, sensitive confirm, dry-run, staged apply and undo as Auto; picks arrive unticked on the receiver with one "Accept all" for safe ones.

**Architecture:** Core owns pick policy (most-specific root resolution, refusals, nested-pick folding, the sensitive-name table, a scan-less send session, the receiver's safe-pick set + `AcceptSafePicks` edit). `carrova-platform` alone writes file-manager entries (Linux/Windows/macOS adapters behind one trait). CLI and Tauri bridge are thin callers; the React flow keeps one selection per mode and sends only the visible one.

**Tech Stack:** Rust 2024 (carrova-core/-platform/-view, clap CLI, Tauri 2 + `tauri-plugin-single-instance`, Windows-only `winreg` + `mslnk`), React + TS (Vitest, Playwright), ts-rs bindings.

**Spec:** `docs/superpowers/specs/2026-11-11-manual-mode-design.md` — sections "User decisions", "All platforms", "Builder-check calls" are binding and override older text.

**Release (user 2026-11-11):** next release = Linux packages with this feature. Windows/macOS right-click code is built and tested in CI but their release jobs stay off until signing is ready.

---

## Global Constraints

- No OS-specific code outside `carrova-platform` (no `cfg(target_os)`, XDG, `$APPIMAGE`, registry, `std::os::*` elsewhere) — add a trait method.
- `src-tauri` holds no business logic; argv parsing/forwarding is plumbing; CLI and UI behave identically.
- Dependencies point down; `carrova-platform` never depends on core.
- No `carrova-crypto` or transport-handshake change.
- Libraries: `thiserror` enums, no `unwrap`/`expect`/`panic!`/`todo!` outside tests (Wave 0 stubs return values). CLI: `anyhow`.
- Tests outside `src/`: `src/a/b.rs` → `tests/a/b/unit.rs` via `#[cfg(test)] #[path = "…"] mod tests;`; integration in `tests/<module>/main.rs`; front-end in `apps/desktop/tests/<module>/…`; Playwright in `apps/desktop/e2e/`. `just test-layout` stays green.
- Test-first for every task.
- Conventional Commits, one focused commit per task, header ≤ 60 chars, no AI attribution. No push without the user. Gate pushes: `just commitlint origin/<branch> HEAD && git push`.
- Bindings: `just bindings` / `just bindings-check`; IPC fixtures: `just ipc-fixtures`.
- All copy in `apps/desktop/src/i18n/en/{send,settings,start,receive}.ts`; no literals in components.
- Every new UI state gets a gallery entry in `apps/desktop/src/gallery/registry/{send,start,receive}.tsx`.
- Quality bar: policies as data tables (one source of truth), pure view-model functions, dumb screens, no `#[allow]`/dead code, docs on new pub items, behaviour-named tests, modules ≤ ~400 lines.
- Machine load: single test targets niced (`CARGO_BUILD_JOBS=3 nice -n 15 cargo test -p <crate> --test <t>`); full `just ci`/e2e only by the main thread, detached; agents check `uptime` before each build and wait if load1 > 10.

## Review Focus

1. **Pick names Carrova cannot carry** (non-UTF-8, control/bidi chars, `\`): `PickRejection::UnsupportedName` at pick time; forwarded argv is lossy → refusal, never crash. Owners: T1 `pick_with_control_char_name_is_refused`, T5 `forwarded_non_utf8_path_is_refused_not_dropped`.
2. **Symlinks** — pick is a link out of roots; a parent is a link (`~/link/proj`); a link inside roots moves as a link. Owner: T1 `pick_symlink_to_outside_is_refused`, `pick_under_linked_parent_is_refused`, `pick_symlink_inside_roots_moves_as_link`.
3. **Pick vanishes/unreadable between pick and capture** → export finishes, item in `Checklist.skipped`. Owner: T1 `export_lists_pick_deleted_after_selection_as_skipped`.
4. **Huge folder sizing** → UI shows a pending row, stays responsive, edits queue. Owners: T6 `pending_row_shows_while_engine_measures`; T1 `shortcut:` comment (measuring not cancellable).
5. **Forwarded `--send` while a move runs / bursts** (Windows `%1` = one process per file; macOS `open --args` doesn't reach a running app) → queue, never alter a running move, show "Finish or cancel the current move first". Owners: T5 `burst_of_forwards_queues_every_path`, `opened_event_queues_paths_macos`; T8 `forwarded_paths_refused_while_sending`.
6. **Thunar `uca.xml`** with user actions / malformed / ours present → only our action (unique id) edited, rest byte-for-byte; malformed = skipped, never overwritten. Owner: T3a.

---

## Waves and file ownership

| Wave | Tasks (parallel) | Depends on |
|---|---|---|
| 0 | T0 contracts (serial, one agent) | — |
| 1 | T1 core sender · T2 core receiver · T3a platform Linux | W0 |
| 2 | T3b platform Windows · T3c platform macOS · T4 CLI | W1 |
| 3 | T5 src-tauri bridge · T6 UI send/Manual · T7 packaging + docs | W2 |
| 4 | T8 UI settings, Home card, receiver Accept-all, forwarded sends | T5, T6 |
| 5 | T9 integrate (bindings, fixtures, CI, e2e, boundary review) | all |

Only T0 edits shared contract files. A later task needing a contract change stops and asks the main thread.

---

## Task T0: Wave 0 contracts (serial)

**Owns:** core `src/secrets.rs` (new, fully implemented), `src/select/pick.rs` (new: types + stubs), `select/mod.rs` (field + `mod pick` + re-exports), `select/edit.rs` (variant), `engine/export.rs` (stub signatures), `plan/model.rs` (field), `plan/edit.rs` (variant + `Offered` field + stub arm), `lib.rs`; platform `src/types.rs`, `platform.rs` (trait defaults), `lib.rs`; view `src/selection.rs` (`PickRow` + `picks: Vec::new()`), `offer.rs` (field `false`), `ipc/mod.rs`, `ipc/reply.rs`, `ipc/settings.rs`, `tests/export_bindings/main.rs`; desktop `src/ipc/{port.ts,tauri.ts,guards.ts,fake/index.ts}` (stubs), `src/engine/{types.ts,host.tsx}`, `src/bindings/*`, `tests/ipc/{tauri.test.ts,guards.test.ts}`.

**Interfaces produced (exact):**

```rust
// carrova-core/src/secrets.rs — one table, sender (picks) + receiver (raw items)
pub enum SecretRule { Name(&'static str), Suffix(&'static str), PrivateKeyPrefix(&'static str) }
pub const SECRET_NAMES: &[(SecretRule, &'static str /* why, user words */)];
// rows: Name(".ssh") "SSH keys", Name(".gnupg") "GnuPG keys", Name(".aws") "cloud credentials",
// Name(".kube") "cluster credentials", PrivateKeyPrefix("id_") (not *.pub) "a private key",
// Suffix(".pem") "a key or certificate", Suffix(".key") "a private key", Name(".env") "environment secrets",
// Name(".netrc") "saved passwords", Name(".pgpass") "saved database passwords"
pub fn secret_in(rel: &str) -> Option<&'static str>; // first match over every '/'-component

// carrova-core/src/select/pick.rs
pub enum PickKind { File, Dir, Symlink }                 // Serialize, TS, snake_case
pub struct PickFacts { pub kind: PickKind, pub size: u64, pub files: u64 }
pub struct Pick { pub root: SemanticRoot, pub rel: String, pub facts: PickFacts, pub sensitive: bool }
impl Pick { pub fn path(&self) -> RootedPath<'_>; pub fn key(&self) -> ItemKey /* ItemKey::raw */ }
#[derive(thiserror::Error, Serialize, …)] #[serde(tag = "kind", rename_all = "snake_case")]
pub enum PickRejection {
  #[error("it no longer exists")] NotFound,
  #[error("it is outside your home and app folders; Carrova only moves files from those")] OutsideRoots,
  #[error("it is your whole {root} folder; pick folders inside it")] IsRoot { root: SemanticRoot },
  #[error("it holds your {root} folder; pick folders inside it")] ContainsRoot { root: SemanticRoot },
  #[error("a folder on its path is a link; pick the real folder instead")] LinkedParent,
  #[error("it is a link to a place outside your home and app folders")] SymlinkOutside,
  #[error("it is not a regular file or folder")] Special,
  #[error("its name has characters Carrova can't carry")] UnsupportedName,
  #[error("Carrova can't read it ({reason})")] Unreadable { reason: String },
}
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum PickResult { Added { item: ItemKey, replaced: Vec<ItemKey> }, Absorbed { into: ItemKey },
                      Duplicate { item: ItemKey }, Rejected { reason: PickRejection } }
pub struct PickOutcome { pub path: String /* input, lossy */, pub result: PickResult }
pub fn locate_pick(platform: &dyn Platform, path: &Path) -> Result<(SemanticRoot, String), PickRejection>; // stub Err(OutsideRoots)
pub fn resolve_pick(platform: &dyn Platform, path: &Path) -> Result<Pick, PickRejection>;                  // stub Err(OutsideRoots)
impl Selection { pub fn add_pick(&mut self, pick: Pick) -> PickResult; }                                  // stub Duplicate

// select/mod.rs
pub struct ItemChoice { …, #[serde(default)] pub pick: Option<PickFacts> }  // Some = a Manual pick
// select/edit.rs
SelectionEdit::RemovePick { item: ItemKey }   // stub arm: Err(unknown item)

// engine/export.rs (stubs return Err(CoreError::Selection(..)) / empty vec)
impl Exporter {
  pub fn start_manual(engine: Engine, cancel: Cancel) -> Result<Self>;
  pub fn scan_now(&mut self, opts: ScanOptions) -> Result<&ScanReport>;
  pub fn pick(&self, selection: &mut Selection, paths: &[PathBuf]) -> Vec<PickOutcome>;
  pub fn pick_key(&self, path: &Path) -> Result<ItemKey, PickRejection>;
}

// plan/model.rs
pub struct Plan { …, /// Recipe-less items whose only sensitivity reason is "unrecognised data".
  #[serde(default)] pub safe_picks: Vec<ItemKey> }
// plan/edit.rs
PlanEdit::AcceptSafePicks          // stub arm: Ok(()) no-op
pub struct Offered { …, pub safe_pick: bool }

// carrova-platform/src/types.rs
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)] #[cfg_attr(feature="ts", derive(ts_rs::TS))]
pub struct FileManagerStatus { pub supported: bool, pub installed: bool, /// e.g. "Thunar: uca.xml is not valid; left unchanged"
  pub skipped: Vec<String> }
// carrova-platform/src/platform.rs (trait, default impls)
fn launcher(&self) -> Result<PathBuf, PlatformError>;   // default current_exe()
fn file_manager_status(&self) -> Result<FileManagerStatus, PlatformError>;   // default Ok(default)
fn install_file_manager_entries(&self, launcher: &Path) -> Result<FileManagerStatus, PlatformError>; // default Unsupported
fn remove_file_manager_entries(&self) -> Result<FileManagerStatus, PlatformError>;                  // default Unsupported

// carrova-view
pub struct PickRow<'a> { pub key: &'a ItemKey, pub display: String /* "~/…" */, pub facts: PickFacts,
                         pub included: bool, pub tags: Vec<SelectionTag<'a>> }
SelectionView { …, pub picks: Vec<PickRow<'a>> }   // rows exclude picks
OfferedItem { …, pub safe_pick: bool }
ipc::Picked<'a> { pub scan: ScanResult<'a>, pub outcomes: &'a [PickOutcome] }  // + owned PickedReply
BridgeEvent::SendRequested            // nudge: call take_send_request
Settings { …, #[serde(default)] pub start_in_manual: bool, #[serde(default)] pub file_manager_asked: bool }
```

```ts
// apps/desktop/src/ipc/port.ts (fake stubs reject with internal)
startManual(): Promise<ScanResult>;
scanApps(session: SessionId): Promise<ScanResult>;
pick(session: SessionId, selection: Selection, paths: string[]): Promise<Picked>;
takeSendRequest(): Promise<string[]>;
fileManagerStatus(): Promise<FileManagerStatus>;
setFileManagerEntries(on: boolean): Promise<FileManagerStatus>;
// tauri.ts commands: start_manual, scan_apps, pick {session,selection,paths}, take_send_request,
//                    file_manager_status, set_file_manager_entries {on}
// engine/host.tsx DesktopHost
readonly pickFiles?: (startIn: string | null) => Promise<string[] | null>;
readonly pickFolders?: (startIn: string | null) => Promise<string[] | null>;
```

- [ ] Failing test `crates/carrova-core/tests/secrets/unit.rs`:
  ```rust
  assert_eq!(secret_in(".ssh/config"), Some("SSH keys"));
  assert!(secret_in("proj/id_ed25519").is_some()); assert!(secret_in("proj/id_ed25519.pub").is_none());
  assert!(secret_in("site/.env").is_some()); assert!(secret_in("notes/keystore.txt").is_none());
  ```
- [ ] `nice -n 15 cargo test -p carrova-core --lib secrets` → compile fail.
- [ ] Implement `secrets.rs`; add all signatures + stubs; extend `export_bindings` (`PickKind, PickFacts, PickRejection, PickResult, PickOutcome, Picked, FileManagerStatus`); `just bindings`; `engine/types.ts` re-exports; `send_requested` in `ipc/guards.ts`.
- [ ] Pass: secrets test, `cargo check --workspace`, `just bindings-check`, `pnpm --filter desktop exec vitest run tests/ipc`.
- [ ] Commit `feat: add manual-mode contracts and bindings`.

---

## Task T1: Core sender — picks, scan-less session, capture

**Owns:** core `select/pick.rs` (bodies), `select/mod.rs` (bodies), `select/edit.rs` (RemovePick arm), `engine/export.rs` (bodies), `capture/items.rs`; view `src/selection.rs` (picks view); tests `crates/carrova-core/tests/select/pick/unit.rs` (new), `tests/manual_pick/main.rs` (new, two engines), `crates/carrova-view/tests/selection/unit.rs`.

**Consumes:** T0 types; `Platform::resolve_root/metadata/read_link/read_dir/open_read_nofollow`; `carrova_platform::path::normalize_abs`; `carrova_manifest::{RelPath, is_unsafe_char}`; `secrets::secret_in`.
**Builds on:** `select/mod.rs:174` `from_scan`; `select/edit.rs:155` unknown-key refusal; `engine/export.rs:76` `start_with`; `capture/items.rs:79` `plan_raw_item` (`ItemFlags::default()` at `:109`); `scan/unrecognised.rs:120` `measure`.

- [ ] Failing unit tests (`FakePlatform`: home `/h`, user_config `/h/.config`, user_data `/h/.local/share`, cache `/h/.cache`, documents `/h/Documents`):
  ```rust
  pick_uses_most_specific_root:   locate_pick(p, "/h/.config/foo") == Ok((UserConfig, "foo"))
  pick_of_a_root_is_refused:      "/h" → Err(IsRoot { root: Home })
  pick_holding_a_root_is_refused: "/h/.local" → Err(ContainsRoot { root: UserData })
  pick_outside_roots_is_refused:  "/etc/hosts" → OutsideRoots; "relative/x" → OutsideRoots
  pick_with_control_char_name_is_refused: "/h/a\u{7}b" → UnsupportedName
  pick_under_linked_parent_is_refused, pick_symlink_to_outside_is_refused,
  pick_symlink_inside_roots_moves_as_link (facts.kind == Symlink), pick_of_fifo_is_special
  pick_of_missing_path_is_not_found
  deep_folder_pick_counts_files_and_bytes: PickFacts { kind: Dir, size: 11, files: 2 }
  pick_containing_ssh_key_is_sensitive: "proj/keys/id_rsa" → resolve_pick("/h/proj").sensitive
  add_pick_absorbs_child: /h/p then /h/p/a → Absorbed { into: raw:home/p }
  add_pick_parent_replaces_children: /h/p/a, /h/p/b, then /h/p → Added { replaced: [a, b] }, 1 item
  add_pick_twice_is_duplicate
  sensitive_pick_starts_unticked; plain pick starts ticked
  remove_pick_drops_only_picks: RemovePick on a recipe key → Err
  ```
- [ ] Run → fail against stubs.
- [ ] `locate_pick`, pinned order: `normalize_abs` (None → OutsideRoots); resolve+normalize every root (skip unavailable); equal to a root → `IsRoot`; a root strictly below → `ContainsRoot`; longest containing root (none → OutsideRoots); rel components `to_str()`, no `is_unsafe_char`, `RelPath::new` ok else `UnsupportedName`; `metadata` on each ancestor between root and path, a symlink → `LinkedParent`.
- [ ] `resolve_pick`: `metadata` (NotFound → NotFound, else Unreadable); symlink → `read_link`, lexical resolve against parent, outside roots → `SymlinkOutside`; `Other` → `Special`; dir → walk `follow_links(false)` summing regular files, `secret_in` per entry rel; file → `open_read_nofollow` error → Unreadable; `sensitive = secret_in(rel) || any entry`. Comment: `// shortcut: measuring is not cancellable; add a Cancel if users pick huge trees`.
- [ ] `add_pick` via `RootedPath::is_within` (absorb + replace); new choice `included: !sensitive, sensitive, paths: vec![path], pick: Some(facts)`. `RemovePick` only when `choice.pick.is_some()`.
- [ ] `Exporter`: `start_manual` = `scan_with(&[], …, ScanOptions { include_undetected: false, unrecognised: false })` then `Session::create(…, State::Scanned)`; `scan_now` replaces `self.scan`; `pick` = resolve + add else Rejected; `pick_key` = locate + `ItemKey::raw`.
- [ ] `capture/items.rs`: `plan_raw_item` gets the choice; `flags: ItemFlags { sensitive: choice.sensitive, ..Default }`; portability stays `SameOs`. Verify `resolve_item` handles a single-file `PathRef`; fix if not.
- [ ] View tests: `picks_are_listed_separately_with_home_display` (`"~/proj/site"`, rows exclude), `unticked_sensitive_pick_stays_visible`, `hidden_raw_does_not_count_picks`; implement (display from `report.platform.roots`, home → `~`; `included_size` adds picks).
- [ ] Two-engine test `tests/manual_pick/main.rs`:
  ```rust
  manual_picks_round_trip_deep_folder_file_and_key  // deep dir + file + .env (confirmed); byte-identical, .env 0o600, manifest sensitive flag
  rejections_are_reported_and_change_nothing        // ["/etc/passwd", "/h"] → [OutsideRoots, IsRoot]
  manual_session_scans_later                         // start_manual → no apps; scan_now → fixture recipe
  export_lists_pick_deleted_after_selection_as_skipped
  picks_from_another_os_are_blocked                  // receiver os=Macos → Blocker::CrossOs
  ```
- [ ] Pass: `--test manual_pick`, `carrova-view --lib selection`.
- [ ] Commit `feat(core): pick files and folders for manual sends`.

---

## Task T2: Core receiver — safe picks and Accept all

**Owns:** core `plan/planner/sensitivity.rs`, `plan/planner/mod.rs` (only to carry `safe_picks`), `plan/edit.rs` (arm + `Plan::offered` fill); view `src/offer.rs`; tests `tests/plan/edit/unit.rs`, `tests/receiver_security/main.rs` (append), receiver goldens (only if `safe_picks` appears).
**Builds on:** `sensitivity.rs:70` `select()`, `:104` `sensitivity()`, `:125` `recipe_reasons`; `dangerous.rs` `DANGEROUS_PATHS`; `options.rs:126`; `edit.rs:178`.

- [ ] Failing tests:
  ```rust
  raw_item_with_only_raw_reason_is_a_safe_pick: plan.safe_picks == [key("home/notes.txt")]
  raw_item_matching_secret_table_is_not_safe: "proj/.env" (sender flag false) → reason "environment secrets", not safe
  raw_item_in_dangerous_path_is_not_safe: home/.bashrc
  archive_flagged_raw_item_is_not_safe
  recipe_items_unchanged_by_secret_table (golden unchanged)
  accept_safe_picks_sets_the_flag_without_confirming_items_one_by_one; accept_safe_picks_adds_to_explicit_list; accept_safe_picks_twice_is_idempotent; accept_all_skips_a_declined_safe_pick
  accept_all_brings_plain_picks_and_stays_judged_on_later_plans; accepted_safe_pick_remapped_into_secret_location_needs_confirmation_again
  ```
- [ ] Run → fail.
- [ ] `sensitivity()` returns `Vec<SensitiveReason>` (join at insert); add `SensitiveReason::Secret { why, at }` for recipe-less items via `secret_in`; `select()` pushes `safe_picks` when `recipe_id.is_none()` and reasons == `[Raw]`; `AcceptSafePicks` sets the option flag `PlanOptions::accept_safe_picks` (never touches `confirmed_sensitive`) and, with an explicit `items` list, ticks every offered safe pick not in `declined`; `PlanOptions::confirms` honours the flag only for keys the current plan counts as safe, so each plan judges it afresh on the final targets (after remaps); items the user declined (`AnswerSensitive` no → `declined`) are never brought back; `remap` to a new place drops the item's one-by-one confirmation; `offer.rs` fills `safe_pick`.
- [ ] Pass incl. `--test receiver_golden` (regenerate per its docs only if a golden changes; inspect diff).
- [ ] Commit `feat(core): accept all safe picks on the receiver`.

---

## Task T3a: Platform — Linux file-manager entries

**Owns:** `crates/carrova-platform/src/linux/file_manager.rs` (+ templates in `src/linux/file_manager/` via `include_str!`), `linux/mod.rs` (`LinuxEnv.appimage`, trait delegation), `tests/linux/file_manager/unit.rs`.

Files written (per user, under resolved user_data / user_config, only these):

| Integration | File |
|---|---|
| Nautilus extension | `<data>/nautilus-python/extensions/carrova_send.py` (`get_file_items(self, *args)`, `args[-1]`, `[]` unless `os.access(LAUNCHER, X_OK)`, local paths only; Popen `[LAUNCHER, "--send", *paths]`) |
| Nautilus script | `<data>/nautilus/scripts/Send with Carrova` (0755, `[ -x L ] \|\| exit 0; exec L --send "$@"`) |
| Nemo | `<data>/nemo/actions/carrova-send.nemo_action` (`Selection=notnone`, `Extensions=any;`, `Dependencies=<launcher>`, `Exec=<launcher> --send %F`) |
| Dolphin | `<data>/kio/servicemenus/carrova-send.desktop` (0755, `MimeType=all/all;`, `TryExec=`) |
| Caja / PCManFM-Qt | `<data>/file-manager/actions/carrova-send.desktop` (`TryExec=`) |
| ~~Open With~~ | removed (2026-11-11): a per-user `MimeType=inode/directory` entry can become the default folder opener where no `mimeapps.list` default exists |
| Thunar | `<config>/Thunar/uca.xml` merged action, `<unique-id>carrova-send-with</unique-id>`, command `sh -c 'test -x "$0" && exec "$0" --send "$@"' '<launcher>' %F` |

Launcher: `$APPIMAGE` (snapshotted in `LinuxEnv`) else `current_exe`; non-UTF-8 or newline → `Unsupported("launcher path")`. Status `installed` = any of our files exists or uca.xml has our id.

- [ ] Failing tests (tempdir `LinuxEnv`): `install_writes_every_entry_pointing_at_launcher`, `exec_quoting_escapes_spaces_and_quotes`, `appimage_env_wins_over_current_exe`, `remove_deletes_only_our_files`, `thunar_merge_keeps_user_actions_byte_for_byte`, `thunar_install_twice_keeps_one_action`, `thunar_malformed_file_is_skipped_not_overwritten`, `thunar_missing_file_is_created`, `status_reports_supported_on_linux`.
- [ ] Run → fail.
- [ ] Implement `exec_quote` (Desktop Entry spec), `xml_escape`, `py_string`; atomic writes (temp + `rename_replace` + `sync_dir`), create parents; Thunar string-level edit (cut our `<action>` by id, insert before last `</actions>`; comment: no XML dep, nothing outside our block touched).
- [ ] Pass. Commit `feat(platform): add Linux file-manager entries`.

## Task T3b: Platform — Windows entries

**Owns:** `src/windows/file_manager.rs`, `windows/mod.rs`, `crates/carrova-platform/Cargo.toml` (Windows-only `winreg`, `mslnk`; workspace entries), `tests/windows/file_manager/unit.rs` (`cfg(windows)`, CI).
Writes `HKCU\Software\Classes\*\shell\Carrova` and `…\Directory\shell\Carrova` (default "Send with Carrova", `Icon=<exe>`, `command` = `"<exe>" --send "%1"`), `%APPDATA%\Microsoft\Windows\SendTo\Carrova.lnk` (target exe, args `--send`). Base key + SendTo dir injectable (tests: `Software\CarrovaTest\<pid>\Classes`, tempdir). Remove deletes only those.

- [ ] Tests: `install_writes_both_verbs_and_sendto`, `remove_deletes_only_our_keys` (sibling survives), `status_tracks_install`. Local: `just check-targets`; real run on CI `windows-latest`.
- [ ] Implement with `winreg` + `mslnk`, no `unsafe`. Commit `feat(platform): add Windows Explorer entries`.

## Task T3c: Platform — macOS Quick Action

**Owns:** `src/macos/file_manager.rs` (+ `Info.plist`, `document.wflow` templates), `macos/mod.rs`, `tests/macos/file_manager/unit.rs` (`cfg(target_os="macos")`).
Writes `~/Library/Services/Send with Carrova.workflow/Contents/{Info.plist,document.wflow}` — Run Shell Script, input as arguments, `open -a Carrova --args --send "$@"`, files + folders in Finder. Services dir injectable via `MacEnv`; remove deletes only the bundle.

- [ ] Tests: `install_writes_quick_action_bundle` (`--send`, `NSServices`), `remove_deletes_only_the_workflow`, `status_tracks_install`. Run on CI macOS / user's M1.
- [ ] Implement. Commit `feat(platform): add macOS Finder quick action`.
- Risk: `open -a … --args` doesn't reach a running app → the Quick Action uses `open -n` (fresh instance → single-instance forwarding; no `RunEvent::Opened`, which would need `cfg(target_os)` in src-tauri); if the M1 check loses paths, switching to `open -a Carrova "$@"` needs user OK.

---

## Task T4: CLI

**Owns:** `apps/cli/src/session/args/selection.rs`, `session/mod.rs` (`start_exporter`, `choose`), `lan/mod.rs` (`--resume` conflicts, `start_send`), `session/args/plan.rs` (`--accept-safe-picks`), `src/integrate.rs` (new), `main.rs` (`Command::Integrate`), `render/selection.rs`; tests `tests/session_args/main.rs`, `tests/cli/main.rs`, `tests/golden/{unit.rs,selection_manual.txt,pick_rejections.txt}`, `tests/session/args/unit.rs`.

```rust
// SelectionArgs
#[arg(value_name = "PATH", conflicts_with_all = ["preset", "include", "exclude"])] paths: Vec<PathBuf>,
#[arg(long, value_name = "ITEM|PATH")] confirm_sensitive: Vec<String>,
pub fn is_manual(&self) -> bool;
pub fn start_exporter(engine: Engine, format: OutputFormat, manual: bool) -> Result<Exporter>;
fn choose_manual(out, exporter: &mut Exporter, a: &SelectionArgs, format) -> Result<usize>;
#[derive(Subcommand)] pub enum IntegrateCommand { FileManager { #[arg(value_enum)] state: OnOffStatus } }
pub fn integrate(out: &mut impl Write, cmd: &IntegrateCommand) -> Result<ExitCode>;
```

`choose_manual`: absolutize (`std::path::absolute`) → `exporter.pick`; each rejection prints `cannot pick <path>: <Display>` and fails before writing; tick picks; `--confirm-sensitive` = key in selection, else `pick_key(path)`, else error `--confirm-sensitive X names nothing you picked`; TTY → `confirm(Consent::Ask, "Send ~/x? It looks like it holds keys or passwords")` per remaining sensitive pick (`--yes` never answers it); then `--exclude-path`/`--force-unmovable`, `exporter.select`.
`integrate`: launcher = `platform.find_binary("carrova-desktop")` else error pointing to desktop Settings; prints `on`/`off`/`not supported here` + skipped lines.

- [ ] Tests: `paths_conflict_with_explicit_preset`, `…_include`, `…_exclude`, `paths_allowed_with_default_preset_and_exclude_path_confirm_sensitive`, `send_resume_conflicts_with_paths`, `integrate_help_lists_on_off_status`, `export_paths_writes_archive_with_picks_only`, `export_path_outside_roots_fails_with_plain_words`, `export_sensitive_pick_without_confirmation_fails_noninteractive`, `export_sensitive_pick_confirmed_by_path_succeeds`, `integrate_file_manager_on_off_status_round_trip`, `import_accept_safe_picks_brings_raw_items`, goldens `selection_manual`, `pick_rejections`.
- [ ] Implement (if clap counts the `preset` default, make it `Option<PresetArg>` with the default in `preset()`). Commit `feat(cli): pick paths for send/export, integrate`.

---

## Task T5: src-tauri bridge

**Owns:** `apps/desktop/src-tauri/src/ops/send.rs`, `commands/send.rs`, `ops/integration.rs` + `commands/integration.rs` (new), `send_request.rs` (new), `state.rs` (`pending_send`), `lib.rs` (single-instance plugin first, startup argv, entry refresh, handlers), `Cargo.toml` + workspace `tauri-plugin-single-instance = "2"`; tests `tests/send_request/unit.rs`, `tests/ops/selection.rs`, `tests/ops/record.rs`.

```rust
pub async fn start_manual(state: &AppState) -> Result<ScanReply, CommandError>;
pub async fn scan_apps(state: &AppState, id: &SessionId) -> Result<ScanReply, CommandError>;
pub async fn pick(state: &AppState, id: &SessionId, selection: Selection, paths: Vec<PathBuf>) -> Result<PickedReply, CommandError>;
pub fn take_send_request(state: &AppState) -> Vec<String>;
pub fn send_paths(argv: &[OsString], cwd: &Path) -> Option<Vec<PathBuf>>;
pub async fn file_manager_status(state: &AppState) -> Result<FileManagerStatus, CommandError>;
pub async fn set_file_manager_entries(state: &AppState, on: bool) -> Result<FileManagerStatus, CommandError>;
pub fn refresh_file_manager_entries(platform: &dyn Platform);  // installed → reinstall with launcher(); logs only
```
Every forward (single-instance callback, startup argv) pushes into `pending_send`, emits `SendRequested`, focuses `main`. The UI decides busy (T8).

- [ ] Tests: `send_paths_takes_everything_after_flag`, `relative_paths_join_cwd`, `bare_flag_is_normal_start`, `no_flag_is_none`, `forwarded_non_utf8_path_is_refused_not_dropped`, `burst_of_forwards_queues_every_path`, `opened_event_queues_paths_macos`, ops `manual_pick_then_export_writes_archive`, `scan_apps_after_manual_returns_suggestion`, `file_manager_status_on_fake_is_unsupported`.
- [ ] Implement; register in `generate_handler!`; `build(ctx)?.run(|app, ev| …)`; `just ipc-fixtures`. Commit `feat(desktop): bridge manual picks and --send forwards`.

---

## Task T6: Desktop UI — Send screen Manual mode

**Owns:** `apps/desktop/src/flows/send/{sendFlow.ts,useSendFlow.ts,sendViews.ts}`, `src/screens/send/{SendFoundScreen.tsx,model.ts,ManualPicks.tsx (new),send.css,fixtures.ts}`, `src/engine/hosts.ts`, `src/ipc/fake/{send.ts,selection.ts}`, `src/i18n/en/send.ts`, `src/gallery/registry/send.tsx`; tests `tests/flows/send/{sendFlow,sendViews}.test.ts`, `tests/screens/send/manual.test.tsx`, `e2e/flows/manual.spec.ts`.

```ts
type SendMode = 'auto' | 'manual';
SendFlowState += { mode: SendMode; byMode: { auto: ScanResult | null; manual: ScanResult | null };
                   pending: readonly string[]; outcomes: readonly PickOutcome[] }
SendController += {
  start(mode?: SendMode, paths?: readonly string[]): void;
  switchMode(mode: SendMode): void;   // first Auto in a manual session → scanApps
  addFiles(): void; addFolders(): void;  // host.pickFiles/pickFolders(startIn = roots.home)
  addPaths(paths: readonly string[]): void; removePick(key: ItemKey): void;
}
export function sendBusy(state: SendFlowState): boolean;
```
Fake dialog host (scenario search param): `/home/ana/projects/site`, `/home/ana/.ssh/id_ed25519`, `/etc/hosts`. Tauri host: `open({ multiple: true, directory, defaultPath })`. Picks go through the same `enqueue` queue as edits.

- [ ] Tests: 'switching modes keeps both lists and shows the other count' ("Manual · 2"), 'next acts on the visible mode only', `pending_row_shows_while_engine_measures`, 'rejected pick is listed with its reason kind'; manual.test.tsx (empty list, buttons, sensitive asks first, remove); e2e manual.spec (switch → Add folder → `~/projects/site`; `/etc/hosts` row "outside your home and app folders"; pairing reachable; accessible).
- [ ] Implement; i18n `send.manual.*` incl. `send.manual.rejected.<kind>` per `PickRejection` kind (Vitest checks every kind has a key); gallery: empty, picks, rejected, sensitive, measuring. Commit `feat(desktop): manual mode on the send screen`.

---

## Task T7: Packaging + docs

**Owns:** `apps/desktop/src-tauri/tauri.conf.json` (`bundle.linux.deb.recommends: ["python3-nautilus"]`, `bundle.linux.rpm.recommends: ["nautilus-python"]`), `packaging/README.md`, `README.md`, `docs/PLAN.md`, `tasks/todo.md`.
`packaging/linux/carrova.desktop` unchanged on purpose (MimeType there would bypass consent; no per-user Open With entry either: it was removed).
Release jobs for macOS/Windows stay off (user 2026-11-11: Linux now, others when signing is ready).

- [ ] Edit; verify `jq . apps/desktop/src-tauri/tauri.conf.json`, `just lint`. Commit `docs: document manual mode and file-manager entries`.

---

## Task T8: Desktop UI — settings, Home card, receiver Accept all, forwarded sends

**Owns:** `apps/desktop/src/flows/settings/{useSettings.ts,settingsViews.ts}`, `src/screens/settings/{SettingsGroups.tsx,model.ts,fixtures.ts}`, `src/screens/start/{HomeScreen.tsx,model.ts,FileManagerBanner.tsx (new)}`, `src/flows/receive/{reviewViews.ts,planOptions.ts}`, `src/screens/receive/ReviewDecisions.tsx`, `src/ipc/fake/planner.ts`, `src/app/shell/routes/useStartFlows.ts`, `src/app/controllers/Controllers.tsx`, `src/i18n/en/{settings,start,receive}.ts`, `src/gallery/registry/{start,receive}.tsx`; tests `tests/{flows/settings,flows/receive/reviewViews.test.ts,screens/start/home.test.tsx,app/flows}`, `e2e/flows/receive.spec.ts`.

Rules: Home card when `!file_manager_asked && supported && !installed && !askReports`; Yes → `setFileManagerEntries(true)` + asked; No → asked. Settings "Sending" group: "Start sends in Manual mode", "Show 'Send with Carrova' in the file manager" (hidden when unsupported; skipped lines as notes). Send starts in `start_in_manual ? 'manual' : 'auto'`. Startup + `send_requested` → `takeSendRequest()`; busy (send or receive in progress) → notice "Finish or cancel the current move first", drop; else navigate to Send, `start('manual', paths)` or `addPaths`. Receiver `safe-picks` decision "Accept all N files" before individual secret decisions → `{ kind: 'accept_safe_picks' }`.

- [ ] Tests: 'home asks once about the file manager', 'answer no never asks again', 'hidden while reports question open', 'settings toggle reflects status and calls setFileManagerEntries', 'start in manual setting opens send in manual', 'forwarded paths open manual send with picks', `forwarded_paths_refused_while_sending`, 'accept all counts only safe unconfirmed picks and emits accept_safe_picks', e2e 'accept all safe picks then approve'.
- [ ] Implement + gallery. Commit `feat(desktop): file-manager setting and accept-all`.

---

## Task T9: Integrate (main thread)

- [ ] `just bindings`, `just bindings-check`, `just ipc-fixtures`; commit `chore: regenerate bindings and IPC fixtures` if changed.
- [ ] Detached + niced: `just ci`, then `just e2e`, `just e2e-ui`; show output.
- [ ] CI `windows-latest` + `macos-latest` green for T3b/T3c; user's M1 manual check of the Quick Action.
- [ ] boundary-reviewer over the diff (no OS code outside platform, no policy in src-tauri, no crypto/handshake diff, test layout, module sizes).
- [ ] CLI/UI parity: rejection words, sensitive confirm, Accept all (`--accept-safe-picks`), integrate on/off/status.

## Main-thread calls recorded

- "Accept all" covers every recipe-less item whose only reason is "unrecognised data" (incl. Auto's unrecognised folders); a pick-only marker would need a manifest format change.
- Windows SendTo shortcut and `mslnk` dropped (unmaintained, internal unwraps); HKCU verbs only.
- macOS Quick Action targets the launcher's `.app` bundle path, not the app name.
