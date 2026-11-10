# Desktop v2 rebuild — handoff plan

Self-contained plan for a fresh session. Start with: *"Read `tasks/v2-rebuild-plan.md` and execute it."*

## 0. Read first

1. `CLAUDE.md`, `tasks/lessons.md`, top of `tasks/todo.md` (Phase 5 decisions + wave notes).
2. Design source: `docs/mockups/gui-v2.html` (open in a browser; navigator, `?theme=dark&dark=a`, deep links `#<screen>:<state>`) and its section in `docs/mockups/README.md`.
3. Approved principles: `docs/research/ui-research.md`.
4. Reference screenshots: `/tmp/carrova-mockups/*--{light,dark}.png` (may be gone after reboot; re-render from the HTML with the repo's Playwright if needed).

## 1. Decisions (all by the user, 2026-11-10 — do not re-ask)

- **Rebuild the whole desktop app to the v2 design** (`docs/mockups/gui-v2.html`).
- **Light** = v2 light tokens (unchanged from v1 values). **Dark = palette A Graphite.** B Ink / C Ember are reference only — do not ship them.
- **"This move" rail** collapses below ~960 px into a header "This move" button that opens it as a side sheet.
- **Welcome screen removed;** Home does onboarding. **Error-report consent** = one-time dismissible card on Home on first run (off until answered; Settings toggle stays).
- **Implement with the `ponytail:ponytail` skill** (smallest complete change; reuse before adding; `shortcut: <limit>, <when to upgrade>` comments). Every implementation brief invokes it. It does not relax the quality bar (§6).
- Mockup design work uses `impeccable` + `ui-ux-pro-max` skills; implementation uses ponytail.
- Commits only when the user asks (Conventional Commits, focused, no AI attribution).

**Main-thread calls already made (user may override, don't re-litigate):**
- Plugin consent is asked on the **receiver** (where the engine runs plugins), not the sender as the mockup shows.
- Cross-OS override copy: the item is "restored to the same place as on <source OS>" — that is what `PlanOptions.allow_cross_os` does. No `~/Carrova/Unplaced/` folder.
- Elements the engine can't feed yet are hidden or rendered in plain words: nearby-computer list, undo expiry date, key fingerprints, plugin signer fingerprint + "Remember", install commands, plugins list in sidebar/Settings.

## 2. Preconditions (check before W3-0)

- [ ] W2-D (plugins) has landed: `crates/carrova-plugin` implemented, `Plan.plugins` / `PlannedPlugin`, `Blocker::UnconfirmedPlugin`, `PlanOptions.confirmed_plugins` exist and bindings are regenerated in `apps/desktop/src/bindings/`. If not, stop and tell the user.
- [ ] `just ci` green on main thread (known: 3 `carrova-core` `receiver_golden` plan-JSON goldens need the new `confirmed_plugins` field re-blessed — fix if still red). Show output.
- [ ] Load monitor running (§7).

## 3. Engine gap closed first — W3-0 (serial, one agent)

The receiver must show the sender in the rail ("ezio-thinkpad · Ubuntu → ezio-mbp · macOS"). Today the receiver only knows its own OS and can derive the source OS from `Plan.mappings` / `Blocker::CrossOs.from`.

- Add the smallest field that carries `{ os: OsKind, device_name: Option<String> }` of the source to what the receiver's review gets (planner suggested `PlanReview.source`; verify the right struct). Source OS is already in the manifest (`SourceInfo.os`); device name must come from the LAN offer/inventory or the archive header — check what exists before adding anything; archive imports may legitimately have `device_name: None`.
- Test-first (two-engine test), ts-rs bindings regenerated, IPC fixtures updated (`just ipc-fixtures` if needed), CLI shows it in plan header if trivial.
- Don't touch `carrova-crypto` or the transport handshake. If the device name would need a handshake/protocol change, stop and ask the user.

## 4. Waves

Paths are under `apps/desktop/src/` unless absolute. Tests mirror src in `apps/desktop/tests/<module>/`; Playwright in `apps/desktop/e2e/`; real-app smoke in `apps/desktop/e2e-app/`.

### W3a — foundation (serial, one agent)

Owns: `theme/*`, `tests/theme/*`, all of `ui/**` + `ui/index.ts`, `app/shell/chrome.ts`, `app/nav/flows.ts` (step labels), `i18n/en/common.ts`, `gallery/Kit.tsx` + gallery shell, `tests/ui/*`.

- **Tokens** (`theme/tokens.ts` stays the one source; light unchanged; dark column → Graphite):

  | Token | Light | Graphite dark |
  |---|---|---|
  | surface-0/1/2/3 | unchanged | `#0D0E11` `#131418` `#1B1D22` `#25272E` |
  | text-primary/secondary/muted | unchanged | `#EDEEF1` `#B6BAC3` `#8D929D` |
  | border / border-strong / control | unchanged | `#2E3139` `#40444F` `#737986` |
  | accent / accent-hover / accent-on / accent-soft | unchanged | `#A3ADFF` `#BCC3FF` `#FFFFFF` `#1E2250` |
  | **accent-fill / accent-fill-hover** (new) | `#3346D3` `#2836B0` | `#5361F0` `#4654E0` |
  | warning, danger, success (+ `-soft`) | unchanged | `#F2B84B/#2F230C`, `#FF8F80/#3A1818`, `#4FD1A0/#0E2D22` |
  | **well** (new) | `#F0F2F5` | `#272A31` |
  | **toast-ok** (new) | `#82C9A5` | `#137A52` |
  | focus, flap, flap-ink, flap-seam, flap-edge, knob | unchanged | `#A3ADFF` `#2A2D36` `#F2F3F5` `#131418` `#3E424D` `#FFFFFF` |
  | scrim, e-1, e-2, e-3, **e-accent** (new) | from mockup light | from mockup `DARK_FX.a` (inset top highlight + shadow) |

  Verify every value against the mockup's token block (search `DEFAULT_DARK`, `DARK_FX`, palette `a`) — the mockup wins over this table.
- **Contrast trap:** dark `accent-on` is white and fails on `--accent`; every filled surface (primary button, checks, current step, toggles) uses `--accent-fill`. In `contrastPairs` swap `accent-on/accent` → `accent-on/accent-fill`, then add the mockup's 33 pairs; all must pass AA in both themes.
- New scales: `t-xs` 12, `t-4xl` 48, `dur-4` 900, `ease-out`, `rail` 296, `side` 212, `h-display` weight 750. Atkinson Hyperlegible Next/Mono already bundled.
- **New components:** `Manifest` (rail: route with OS badges, facts with Change links, promise; collapses <960 px to header button + sheet), `Stepper` (labelled; replaces `StepDots`), `OsBadge`, `MapRow` (highlights the changed path segment), `PermRow`, `Seal` (one-off burst), `Toast`; `Timeline` rewritten (vertical steps + move card).
- **Changed:** Button (primary = fill + `e-accent`), Check (tri-state + warn), Toggle, Segmented, Badge (→ v2 chip), Banner, List/Row, Decision, Tree, AppGlyph (`well`), Progress (track + sheen); `FlowLayout` (head with Stepper + route chip, `rail` prop, action bar), `HubLayout` (sidebar, bundle status at foot), Sheet/Dialog/Window restyle, `FlowFrame` (step labels, `stepNow` override).
- **Delete** (after callers move — may defer deletion to integration): `Chip`/`Chips`, `Arrival`, `BareLayout`, `StepDots`.
- Every animation has a reduced-motion fallback (numbers still update).
- **Exit:** `just ci` green with the old screens rendered on the new primitives; Kit page matches mockup `kit` screen in both themes.

### W3b / W3c / W3d — parallel (≤3 agents, disjoint files)

**W3b — Send / Pair / Save.** Owns: `screens/send/*` (except `SendLiveScreen`, `SendDoneScreen`), `screens/file/*`, `flows/send/*`, `flows/archive/*`, `routes/SendRoutes.tsx`, `routes/SendOverlays.tsx`, `i18n/en/{send,file}.ts`, `gallery/registry/{send,file}.tsx`, `tests/flows/{send,archive}`, `e2e/flows/{send,archive}.spec.ts`.
- Send found: **rewrite** (`SendFoundScreen.tsx`, `FoundSelection.tsx`) — category groups, keys-and-secrets first with warn band, start unticked.
- **New** `screens/send/KeyConfirmDialog.tsx` (per-key confirm; list key files; fingerprints hidden until engine supplies them).
- Customize sheet: **restyle** (`CustomizeSheet.tsx`, `CustomizeTree.tsx`).
- Pair waiting/handshake/linked: **restyle** `SendCodeScreen.tsx`, `ui/CodeDisplay`; **rewrite** `ui/Handshake` (code tiles flip in, lock clicks shut); **new** linked cross-OS banner. Coordinate: `ui/*` changes beyond Handshake/CodeDisplay go through W3a's API, not edits.
- Receive code screen (`screens/receive/RecvCodeScreen.tsx`) restyle belongs to W3c.

**W3c — Review / Transfer / Done.** Owns: `screens/{receive,override,shared}/*`, `screens/send/SendLiveScreen.tsx`, `screens/send/SendDoneScreen.tsx`, `flows/receive/*`, `flows/shared/*`, `routes/{ReceiveRoutes,ReceiveOverlays,OverrideRoute,autoOverlays}.tsx`, `ipc/fake/*`, `i18n/en/{receive,override,transfer}.ts`, `gallery/registry/{receive,override}.tsx`, `tests/flows/{receive,shared}`, `e2e/flows/receive.spec.ts`.
- Review needs/ready: **rewrite** `RecvReviewScreen.tsx`, `ReviewDecisions.tsx`, `SpaceRow.tsx` — counts row, space meter, "Needs your answer", "Suggested — change anything". Replace W2-D's interim mapping/plugin banners with decision rows.
- **New** `screens/receive/MappingSheet.tsx` (from `Plan.mappings`; source path above target path, changed segment highlighted; "same place" items collapsed).
- Override: **restyle** `screens/override/OverrideDialog.tsx`; add `cross_os` to `OverrideKind`; primary disabled until "I understand" is ticked; writes `allow_cross_os`.
- **New** `screens/receive/PluginConsentDialog.tsx` from a decision row (data: `Plan.plugins` → id, version, description, permission sentences, items); writes `confirmed_plugins`. "Allow for this move" only (no Remember).
- Transfer live/interrupted: **rewrite** `screens/shared/TransferBody.tsx`, `RecvProgressScreen.tsx`, `SendLiveScreen.tsx` — live card (%, bytes, ETA, rate, encrypted), apps smallest-first ticking off, finished apps fold on interruption, paused state says what's safe. ETA/rate computed front-end in `flows/shared/transferView.ts` (`secondsLeft` is null today).
- Done: **rewrite** `RecvDoneScreen.tsx` + `LeftForYou.tsx` (Seal, summary, reinstall checklist without commands, rollback link); **restyle** `SendDoneScreen.tsx`.
- `RecvCodeScreen.tsx` restyle.
- `TransferModel` changes must be **additive** (W3b routes render these screens).
- New fake IPC scenarios `cross-os` and `plugin` in `ipc/fake/scenario.ts`; re-record fixtures with `just ipc-fixtures` (one src-tauri compile at a time).

**W3d — Hub.** Owns: `screens/{start,history,undo,settings,apps}/*`, `flows/{history,undo,settings,catalog}/*`, `app/nav/navMachine.ts`, `app/Startup.tsx`, `routes/{HubRoutes,HubOverlays,UndoRoute,useStartFlows}.ts*`, `i18n/en/{start,history,undo,settings,apps}.ts`, `gallery/registry/{hub,start}.tsx`, `tests/app/*`, `e2e/smoke.spec.ts`, `e2e/flows/support.ts`.
- Home first-run/returning: **rewrite** `screens/start/HomeScreen.tsx` (two verb tiles with S/R shortcuts, dismissible hint, 3-step strip, promise line, "Open an archive file…", recent sessions when returning, one-time error-report consent card).
- **Delete** `WelcomeScreen.tsx`; remove the `welcome` view and `finishWelcome` from `navMachine.ts`; consent card shows while consent is `unset` (`app/Startup.tsx`).
- History timeline/preview/undone: **rewrite** `screens/history/HistoryScreen.tsx` as two panes; **delete** `SessionSheet.tsx`; **restyle** `screens/undo/*` (`UndoConfirmDialog` shows exact counts incl. files edited since the move); `rolled_back` record shows the rewind.
- Settings main/update: **restyle** `screens/settings/*`. Apps/About/crash screens: restyle via tokens only.
- `e2e/flows/support.ts`: `openApp` no longer clicks "Get started"; h1 copy changes.

### Integration (one agent, after W3b–d)

- Delete dead code (WelcomeScreen, SessionSheet, Chip, Arrival, BareLayout, StepDots) once no callers.
- `e2e/gallery.spec.ts` target count + kit, axe in light and Graphite; `e2e-app/specs/smoke.e2e.ts`; pseudo-locale pass (no clipped strings, RTL-ready).
- `apps/desktop/src-tauri/tauri.conf.json` window width 1100 (config only; min width stays 720 — rail collapse covers it).
- Visual pass: each gallery state vs `gui-v2.html#<screen>:<state>` in light + Graphite, by eye (no pixel baselines — fonts differ by OS). Use the `impeccable:impeccable-finish-reviewer` agent for an independent critique.
- `boundary-reviewer` agent (no business logic in `src-tauri`; UI never pokes engine state).
- `just ci` green, output shown. Update `tasks/todo.md` (W3-G done) and `docs/PLAN.md` status.

## 5. Test strategy

- View-models first, test-driven: new `tests/flows/{send,receive,shared}/…{manifestViews,mappingViews,transferView}.test.ts`; extend `reviewViews.test.ts` for plugin + cross-OS decisions.
- Update: `tests/theme/contrast.test.ts`, `tests/ui/{controls,overlays,motion}.test.tsx`, `tests/screens/{screens.test.tsx,logic.test.ts}`, `tests/app/nav/navMachine.test.ts` (Welcome removal), `tests/app/flows/*.app.test.tsx`, `App.test.tsx`.
- Playwright: copy changes in send/receive/archive specs; new specs for mapping sheet, cross-OS override, plugin consent, rail collapse at <960 px, first-run consent card.
- Recorded `ipc/fixtures/*` stay as recorded except the new cross-os/plugin scenarios; `screens/*/fixtures.ts` get new view-model fields.
- `just test-layout` enforces test placement (no tests under `src/`).

## 6. Quality bar (put in every brief)

Senior React/TS: TS strict, small components, pure view-models separated from rendering, one source of truth for tokens and copy (i18n keys, no hard-coded strings), no dead code, no `eslint-disable` without a reason, accessible (labels, focus order, focus ring, keyboard paths, `aria-live` for progress), WCAG AA measured, reduced motion. Bindings are generated — never hand-edit. `src-tauri` gets no business logic. Delight = feedback moments (code flip, lock click, ticks, seal, rewind), never dark patterns; consequences never hidden, only advanced controls.

## 7. Machine budget (12 cores, ~13 GiB RAM, disk tight ~29 GB free)

- Logger: `just load-watch` → `/tmp/carrova-load.log`. Alert filter `/tmp/carrova-load-alert.awk` (load1 > 9, mem < 3 GiB, disk < 8 GiB) armed via Monitor: `tail -n0 -F /tmp/carrova-load.log | mawk -W interactive -f /tmp/carrova-load-alert.awk` (prove it with a fake bad line first; re-arm every 30 min). Recreate the awk file if `/tmp` was cleared.
- ≤3 concurrent agents; launch only if load1 < 9, mem > 3 GiB, disk > 8 GiB. At most **one** `src-tauri` compile at a time. Agents use `pnpm --filter desktop …` and per-crate cargo; workspace `just ci` only on main thread at wave end.

## 8. Done

- [x] W3-0 source device/OS on receiver (`PlanReview.source`; name from sender manifest `SourceInfo.device_name`)
- [x] W3a foundation (desktop tsc + vitest 364 green)
- [x] W3b Send/Pair/Save (own scope + send/archive e2e green)
- [x] W3c Review/Transfer/Done (desktop tsc+eslint clean, vitest 409 green, 6 receive e2e green; fixtures re-recorded)
- [x] W3d Hub (own scope green; HubStatus tone gap → integration)
- [x] Integration + visual pass + boundary review + `just ci` green (output shown)
- [x] `tasks/todo.md` + `docs/PLAN.md` updated; real-app screenshots (light + Graphite) in `/tmp/carrova-visual/real/`

## 9. Session-2 status + handoff (2026-11-10)

Preconditions met: `just ci` green before W3 (exit 0). Done: W3-0, W3a, W3b, W3c, W3d (checked in §8). All wave agents finished; main thread verified desktop tsc + eslint clean, vitest 46 files / 409 tests green. **Next = Integration only.** W3c left: `send.done.stamp` (send.ts) unused; Graphite not checked by eye for W3c screens; Done lists app ids not names (`app_names` covers only missing/running); mapping source side shows folder type, not real path. Playwright needs `vite build --mode e2e` first (server reuses stale `dist-e2e`). A `vite preview --outDir dist-gallery --port 4179` server from an earlier session is still running (harmless).

**W3-0 result:** `PlanReview.source: SourceDevice | null` (`{os, device_name}`), name from sender capture → `SourceInfo.device_name` in manifest (validated: no control/bidi chars, ≤ `MAX_DEVICE_NAME` 64). Receiver record `Source::Lan.peer_name` filled from it. CLI plan header `from: <name> (<OS>)`. No crypto/handshake change. **Open for user:** archives now carry sender host name (encrypted); OS-only is the alternative.

**W3c brief (if it must be redone/finished):** plan §4 W3c, plus: owns full `just ipc-fixtures` re-record (W3-0 found ~718 lines of genuine drift from earlier waves; hand-added only `source`); give the recorded fake sender a device name if trivial; delete `SendLiveScreen`'s unreachable "reviewing" branch (W3b added `SendLinkedScreen`); reuse `screens/send/SendRail.tsx` (`osName`, `manifestEnd`, `RouteChip`) and `settings.about.os.*` keys for the receiver rail. Red at last check, all W3c-owned: `reviewViews.ts:80` crash ("reading 'bash/config'") breaking 13–21 receive tests incl. `tests/app` receive/archive-open/moveDefaults/transferControls; tsc in `tests/flows/receive/reviewViews.test.ts` + receive cases of `tests/screens/screens.test.tsx`; eslint `src/flows/shared/useTransferRate.ts`; prettier on 9 W3c files.

**Integration todo (collected from wave reports):**
- `HubStatus` needs a tone — `HubLayout` foot shield is always green, even "Recipes not signed" (W3d).
- `GroupHead` needs a lead slot for category select-all checkbox (W3b deferred; heading a11y name).
- `e2e/gallery.spec.ts`: target counts (welcome + session removed; new `kit`, send/pair/history ids now follow mockup) + axe in light and Graphite.
- Delete dead code once callerless: `Chip`/`Chips`, `Arrival`, `BareLayout` (StepDots, WelcomeScreen, SessionSheet already gone).
- `tauri.conf.json` window width 1100; rail collapse is a viewport media query (fine in Tauri, not inside gallery frame).
- Sensitive-reason copy from engine reads "marked sensitive by recipe ssh/keys" — recipes should carry human wording.
- Home file link kept as "Use an encrypted file instead" (not spec's "Open an archive file…") so Save-to-file stays reachable — user to confirm.
- "Start here" hint dismissal is in-memory only (`shortcut:` noted).
- Deferred for missing engine data: undo expiry, OS badges on history rows (`SessionRecord` has no peer OS), per-file key lists, distro/version, nearby computers, fingerprints, plugins list.
- Then: `e2e-app/specs/smoke.e2e.ts`, pseudo-locale pass, visual pass vs `gui-v2.html#<screen>:<state>` (light + Graphite) with `impeccable:impeccable-finish-reviewer`, `boundary-reviewer`, `just ci` green (output shown), update `tasks/todo.md` W3-G + `docs/PLAN.md`, screenshots of real app to user.

**Machine:** `just load-watch` logger is running (PID of `just load-watch` shell); alert filter `/tmp/carrova-load-alert.awk` proven. Lingering cargo/`pgrep -f` wait-loops from agents can self-match forever — check `pgrep -af cargo` at start.
