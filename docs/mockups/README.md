# Carrova UI mockups

These are design mockups, not the real app. They're static, self-contained HTML files with no build step, no network requests and no dependencies.

- **`gui.html`:** the desktop app (Phase 4).
- **`gui-v2.html`:** the premium pass on the desktop app, with Phase 5 cross-OS and plugin consent. See [v2](#v2-gui-v2html-premium-pass).
- **`tui.html`:** a terminal UI in ratatui style, 100×30.
- **Research behind this round:** [`docs/research/ui-research.md`](../research/ui-research.md).

## What changed and why

The verdict on round one was: "the UI is not looking good, too complicated". The [research](../research/ui-research.md) points at the products that make this feel effortless: Pixel's **Copy / Customise**, Apple Migration Assistant's categories-then-items, Warp's code-only pairing and Linear's calmer chrome. They all hide *controls* behind a secondary path and never hide *consequences*.

| | Round 1 | Now |
|---|---|---|
| Send happy path | 6 steps: scan, choose (Simple or Advanced), pair, their review, transfer, done | **4 screens:** Here's what we found, Pairing code, Live (their review becomes progress), Delivered |
| Receive happy path | 5 steps: code (plus a connecting screen), plan, transfer, apply, checklist | **4 screens:** Type the code (handshake inline), Here's what will change, Moving (apply is its last phase), Arrived |
| Onboarding | 3 screens | 1 screen |
| Product chrome | A step rail, a status bar with key hints and phase badges on every screen | A quiet top bar on hub pages. Flows show only close, step dots and one action bar. Phase badges appear only in the dev **Screen index** (press `i`). |
| Detail | Inline on the main path (the Advanced tree, per-file rules, overrides) | In **sheets** (Customize, Every change, Unrecognised folders, Move details, App detail) and one **override dialog** |

"User decides everything" is intact. Every default is written in words with a way to change it: suggested ticks, "back up and replace (suggested)", undo retention, caches, lock strength. Sensitive items are asked once on each side, in plain words, and start on **Leave behind**. Approve stays disabled until every decision has an answer.

**Delight**, with one signature moment per flow:
- the split-flap pairing code
- the handshake lock clicking in
- the "Arrived" stamp with a small burst
- app chips ticking off small-first, in the engine's real transfer order

Every animation stops under `prefers-reduced-motion` or the **Reduce motion** toggle, and progress still updates. We rejected these engagement patterns: streaks, nagging, confirmshaming, fake progress and hidden cancel. The research note says why.

## How to open

- **Desktop:** open `gui.html`.
  - The dev bar has **Screen index** (`i`), **Prev/Next** (`[` `]`), **Dark**, **Right to left** and **Reduce motion**.
  - State chips above the window switch between the states of the current screen.
  - Deep links look like `gui.html#recv-progress:paused`.
  - `Esc` closes a sheet or dialog.
- **Terminal:** open `tui.html`. Use `←` and `→` to step through screens. Deep links look like `tui.html#tui-recv-review`.

## Screens

| Area | Desktop (`gui.html`) | Terminal (`tui.html`) |
|---|---|---|
| Start | Welcome (first run), Home (returning, no moves yet), Use a file instead (dialog) | Welcome, Home |
| Send | Here's what we found (scanning, found, nothing found); Customize (sheet); Folders Carrova doesn't recognise (sheet); Pairing code (waiting, can't connect, wrong code tried, 3 wrong codes); Connected, then sending (reviewing, moving, reconnecting); Delivered | Here's what we found, ‹Customize› drill-in, pairing code (waiting, can't connect, wrong code, stopped), connected, sending, delivered |
| Receive | Type the code (typing with word autocomplete, connecting handshake, not found, didn't match); Here's what will change (needs answers, ready); Every change (sheet); Moving (moving, disk full, reconnecting, putting in place, failed and undone); Arrived (all, some failed); Undo preview (dialog); Undone (complete, partly failed) | Same, plus the ‹Every change› drill-in, override popup and undo preview |
| Encrypted file | Save: what goes in, Save: passphrase (strong, too weak), Save: saved, Open a file (ready, wrong passphrase). Then the receive screens. | Save passphrase, Open a file |
| History | History (with moves, empty), Move details (sheet: timeline, redacted log, undo point) | History, empty, move details |
| Apps Carrova knows | Catalogue (all, community), App detail (sheet) | Catalogue |
| Settings | Settings, Plugins (P5), Plugin permissions (dialog, P5), Update ready (dialog), Error report (dialog), About and licences (sheet) | Settings, Plugins, Update, Error report |
| Overrides | One dialog with 5 states: unmovable item, app running, free space, somewhere else, unsigned plugin | Override popup |
| Moving between systems (P5) | Moving to Windows: where things land, and what can't move and why | Same |
| Design system | Tokens, type, spacing, layouts, every component, motion and composition rules | (uses the same palette) |

Every error state has the same shape: what happened, why, and **one** recovery action.

## Design system

The **Design system** screen in `gui.html` is the source of truth. It reads the live tokens, so it can't drift from them. New screens are *composed*:

1. Pick a layout:
   - **Hub** for places.
   - **Flow** for tasks: close, step dots, a centred 36rem column and an action bar.
   - **Sheet** for drill-down detail.
   - **Dialog** for a risk or confirmation.
2. Compose components:
   - button (primary, secondary, quiet, danger)
   - choice tile, card, list row, decision
   - toggle, checkbox, segmented choice
   - badge, banner, stat, progress, app chip
   - code tag, handshake, arrival stamp
   - copy box, disclosure, empty/error state, lock line, key hint

   They're JS functions returning markup, and each has one CSS block.
3. One primary button per screen. Its label says what happens.

Scales:
- **Spacing:** an 8-pt grid (`--sp-0…--sp-8`: 4, 8, 16, 24, 32, 40, 48, 64).
- **Type:** ratio 1.25 (13, 15, 18, 22, 28, 36).
- **Radius by hierarchy:** control 8, card 14, sheet 20.
- **Motion:** 120, 220 and 420 ms, with a spring only where something lands.
- **Layout:** logical properties only, so right-to-left mirrors for free. Paths and codes are forced left-to-right.

### Colour tokens (light and dark)

Dark follows Material, Apple HIG and Primer practice:
- a near-black neutral base and no `#000`
- elevation from **lighter surfaces**, not shadows
- text at about 90%, 68% and 55% white
- borders at about 10% white
- desaturated accent and status colours

The theme follows `prefers-color-scheme` live until you use the toggle. See the [research note](../research/ui-research.md#6-dark-theme-before-and-after) for the before and after.

| Token | Light | Dark | Role |
|---|---|---|---|
| `surface-0` | `#F0F2F5` | `#101216` | Wells: inputs, segmented track, logs |
| `surface-1` | `#F7F8FA` | `#15171C` | Window base |
| `surface-2` | `#FFFFFF` | `#1C1E24` | Cards, rows, chips |
| `surface-3` | `#FFFFFF` | `#23262D` | Sheets, dialogs |
| `text-primary` | `#15181D` | `#E4E6EA` | Body, headings |
| `text-secondary` | `#3E4552` | `#B0B4BC` | Supporting copy |
| `text-muted` | `#5E6672` | `#8E939D` | Metadata, hints |
| `border` | `#E3E6EB` | `#2C2F35` | Decorative dividers |
| `border-strong` | `#CDD2DA` | `#3C4048` | Card and window edges |
| `control` | `#858C98` | `#6E747F` | Edges of inputs, checkboxes, toggles |
| `accent` | `#3346D3` | `#A4ADEB` | The only "act" colour |
| `accent-hover` | `#2836B0` | `#BAC1F2` | |
| `accent-on` | `#FFFFFF` | `#151830` | Text on the accent |
| `accent-soft` | `#EEF0FD` | `#262A45` | Selection, info banners |
| `warning` / `-soft` | `#8A5A00` / `#FFF4DC` | `#DDB76A` / `#2A2519` | Sensitive items |
| `danger` / `-soft` | `#B42318` / `#FDECEA` | `#EC9C92` / `#2F1F1E` | Destructive, failed |
| `success` / `-soft` | `#1F7A4D` / `#E8F5EE` | `#82C9A5` / `#1A2B23` | Arrived, verified, encrypted |
| `focus` | `#3346D3` | `#A4ADEB` | Focus ring |

The pairing-code tiles use `flap` (`#1B1E24` light, `#2A2D35` dark) with `flap-ink`. The TUI terminal palettes reuse the same hex values.

**Measured contrast** (WCAG 2.x relative luminance; body text needs 4.5:1, UI edges and focus need 3:1):

| Pair | Needs | Light | Dark |
|---|---|---|---|
| `text-primary` on `surface-1` | 4.5:1 | 16.74 ✓ | 14.35 ✓ |
| `text-primary` on `surface-3` | 4.5:1 | 17.79 ✓ | 12.12 ✓ |
| `text-secondary` on `surface-1` | 4.5:1 | 9.07 ✓ | 8.62 ✓ |
| `text-secondary` on `surface-3` | 4.5:1 | 9.64 ✓ | 7.28 ✓ |
| `text-muted` on `surface-1` | 4.5:1 | 5.46 ✓ | 5.81 ✓ |
| `text-muted` on `surface-2` | 4.5:1 | 5.80 ✓ | 5.40 ✓ |
| `text-muted` on `surface-3` | 4.5:1 | 5.80 ✓ | 4.91 ✓ |
| `text-muted` on `surface-0` | 4.5:1 | 5.17 ✓ | 6.08 ✓ |
| `accent` on `surface-1` | 4.5:1 | 6.68 ✓ | 8.33 ✓ |
| `accent` on `surface-3` | 4.5:1 | 7.10 ✓ | 7.04 ✓ |
| `accent-on` on `accent` | 4.5:1 | 7.10 ✓ | 8.09 ✓ |
| `accent-on` on `accent-hover` | 4.5:1 | 9.35 ✓ | 9.96 ✓ |
| `accent` on `accent-soft` | 4.5:1 | 6.26 ✓ | 6.51 ✓ |
| `warning` on `warning-soft` | 4.5:1 | 5.43 ✓ | 8.03 ✓ |
| `warning` on `surface-2` | 4.5:1 | 5.93 ✓ | 8.77 ✓ |
| `danger` on `danger-soft` | 4.5:1 | 5.75 ✓ | 7.29 ✓ |
| `danger` on `surface-2` | 4.5:1 | 6.57 ✓ | 7.72 ✓ |
| `success` on `success-soft` | 4.5:1 | 4.74 ✓ | 7.67 ✓ |
| `success` on `surface-2` | 4.5:1 | 5.32 ✓ | 8.61 ✓ |
| `flap-ink` on `flap` | 4.5:1 | 15.71 ✓ | 11.02 ✓ |
| `control` on `surface-2` | 3:1 | 3.39 ✓ | 3.54 ✓ |
| `focus` on `surface-1` | 3:1 | 6.68 ✓ | 8.33 ✓ |
| `focus` on `surface-3` | 3:1 | 7.10 ✓ | 7.04 ✓ |

TUI terminal palettes: `dim` text on `bg`, `selection` and `popup` is ≥ 4.54:1 in dark and ≥ 4.73:1 in light. Tile letters measure 11.0:1 in dark and 13.6:1 in light.

## v2: `gui-v2.html` (premium pass)

`gui-v2.html` is a second desktop mockup that builds on v1 instead of replacing it. `gui.html` is unchanged. It's one self-contained file. A navigator on the left lists every screen and state, and there are **Dark**, **Reduce motion** and **Replay** toggles. Keys: `[` and `]` step through screens, `t` switches theme, `r` replays, `Esc` closes an overlay. Deep links look like `gui-v2.html#recv-review:mapping` and `?theme=dark`. Fonts load from the workspace's `node_modules` after `pnpm install`; otherwise it falls back to the system font.

**Concept: the move has a manifest.** Every flow screen has a right-hand rail called **This move**. It shows the route (this computer → the other one, each with an OS badge) and every default in words, each with a Change link: keys, caches, backups, undo window. It fills in as you go: paired, reviewing, receiving, arrived. The main column holds the one decision that screen is about. The manifest is the promise kept visible: what moves, where, and how to undo it.

What changed from v1:

| | v1 | v2 |
|---|---|---|
| Flow chrome | Close, step dots, one centred 36rem column | Close, **labelled steps** (Pick · Pair · Their review · Move), a route chip, the main column, and the **manifest rail**. It uses the full 1100 px window. |
| Hub | Quiet top bar | Quiet sidebar: Home, History, Apps, Plugins and Settings, with recipe-bundle status at the foot |
| First run | A separate welcome screen | Home is the onboarding: two verb tiles, one dismissible hint ("Start here on your old computer"), a three-step strip that names the whole task, and the promise. Learnable in under a minute, with no tour. |
| Send | Rows plus sensitive questions | Category groups with group ticks. **Keys and secrets** come first, start off, and each one opens a confirm that lists the key files and fingerprints. Customize opens a tri-state file tree in a sheet. |
| Pairing | Code, then a handshake | The code flips in. **Nearby computers** show an OS badge and a live status ("Typing the code…"). The handshake is a moment of its own: both machines slide in, the wire draws, the lock clicks shut, then a cross-OS heads-up. |
| Review | Counts plus decisions | Counts, a **space meter**, then *Needs your answer* (sensitive item, running app, can't-cross-OS item) and *Suggested, change anything* |
| Cross-OS (P5) | A separate screen | A **path-mapping sheet** inside the review: each from/to pair has OS badges, the changed segment is highlighted, and "same place" shows where nothing changes. A **can't-move override** sits behind a warning that explains why, and its button stays off until you tick "I understand". |
| Plugins (P5) | A permissions dialog | A consent dialog modelled on the iOS and 1Password prompts: the signer and a key fingerprint, one plain line per permission, and the reassuring **absences** stated outright ("No internet", "can't see your keys"). Allow covers this move only unless you tick Remember. |
| Transfer | Chips ticking | A live card (big %, MB, a real ETA, rate, and the wire with a sheen) and an app list ordered **smallest first**, so ticks land within seconds. When the transfer is interrupted, finished apps fold into one row and the paused one says exactly what's safe. |
| Done | A stamp | A rotating **"Arrived · all checked" seal** with a one-off burst, a receipt (files, backups, problems), and a **Finish setting up** checklist with copyable install commands |
| History | A list plus a sheet | A **timeline** with step-by-step detail beside it. Undo first shows exact counts, including files you edited since, which are kept as a copy. After it runs, the steps **rewind** in reverse order. |
| Design system | A screen in the mockup | A **component sheet** with swatches (light and dark from one table), live contrast measurements, the type scale, buttons, chips, OS badges, rows, banners, toasts, empty states, the sheet and dialog, the code tiles and the manifest |

The light colour tokens are v1's values, unchanged. New tokens are `toast-ok` (the check on the inverted toast), `accent-fill` / `accent-fill-hover` and `well`; each one's light value equals an existing token. The dark values were redone in round 2 (see below). Every pair the page renders passes WCAG AA in both themes; the component sheet recomputes these numbers from the token table. The new pairs are:

| Pair | Needs | Light | Dark |
|---|---|---|---|
| `text-primary` on `surface-2` | 4.5:1 | 17.79 ✓ | 13.33 ✓ |
| `text-secondary` on `surface-2` | 4.5:1 | 9.64 ✓ | 8.01 ✓ |
| `text-secondary` on `surface-0` (sidebar) | 4.5:1 | 8.60 ✓ | 9.02 ✓ |
| `accent` on `surface-2` | 4.5:1 | 7.10 ✓ | 7.74 ✓ |
| `surface-1` on `danger` | 4.5:1 | 6.19 ✓ | 8.31 ✓ |
| `surface-1` on `text-primary` (toast) | 4.5:1 | 16.74 ✓ | 14.35 ✓ |
| `toast-ok` on `text-primary` | 3:1 | 9.19 ✓ | 4.26 ✓ |

#### Dark palettes (round 2)

Feedback: "the light colors are looking good, but the dark colors are not." Light values are unchanged. Here's what was wrong with the old dark theme, which reused v1's dark tokens:
- **The primary button read as disabled.** It was a pastel periwinkle (`#A4ADEB`) with dark text, and it looked nearly the same as the real disabled state at 45% opacity.
- **The surfaces were flat grey on grey.** `#101216`, `#15171C`, `#1C1E24` and `#23262D` are lightness steps of about 0.3%, so cards hardly lifted, and with no shadows there was no depth.
- **Borders disappeared on sheets and dialogs.** `#2C2F35` on `#23262D` measures 1.07:1, so the dividers in the mapping sheet and dialogs couldn't be seen.
- **The soft fills were muddy.** Success, warning and danger soft fills were grey-brown and the accent soft fill was grey-violet, so chips and banners looked dirty rather than tinted.
- **App glyph tiles looked like black holes** inside cards, because the wells were darker than every card.

Fixes shared by all three directions:
- A new **`accent-fill`** and **`accent-fill-hover`**. They're a saturated mid-tone with white text, used for the primary button, checks, the current step, toggles and the code number. Light keeps `#3346D3` and `#2836B0`, so nothing changes there.
- `accent` stays a light tint for links and icons.
- Bigger elevation steps.
- Shadows plus a 1 px top highlight in dark, and an offset accent shadow under the primary button.
- Borders that are visible on `surface-3`.
- Chroma-rich soft fills.
- A **`well`** token for glyph tiles, chips and permission icons, raised in dark and equal to `surface-0` in light.
- Semantic colours are separated by hue and by lightness, and always come with an icon, so they stay apart for colour-blind users: teal-green for success, amber for warning, coral for danger.

Use the navigator's **Dark palette** switch, or `?dark=a|b|c`:

| | Surfaces 0 / 1 / 2 / 3 | Text 1 / 2 / 3 | Accent / fill | Success · warning · danger |
|---|---|---|---|---|
| **A Graphite** (default) | `#0D0E11` `#131418` `#1B1D22` `#25272E` | `#EDEEF1` `#B6BAC3` `#8D929D` | `#A3ADFF` / `#5361F0` | `#4FD1A0` · `#F2B84B` · `#FF8F80` |
| **B Ink** | `#090D16` `#0E131E` `#161C2A` `#1F2637` | `#E9EDF5` `#AFB8CA` `#8791A6` | `#8FB4FF` / `#2F6BEA` | `#4FD8B0` · `#F5C46B` · `#FF9090` |
| **C Ember** | `#100F0D` `#171513` `#201E1B` `#2A2724` | `#F2EEE8` `#C0B9AF` `#9C948A` | `#B3ABFF` / `#6152E0` | `#72CF95` · `#EDB55A` · `#F79483` |

All 33 rendered pairs pass AA in each direction; the component sheet recomputes them for whichever palette is active. The lowest text pairs are:
- **Graphite:** muted on `surface-3` 4.78, and white on the fill 4.84.
- **Ink:** white on the fill 4.76.
- **Ember:** muted on `surface-3` 4.96.

**Decision (user, 2026-11-10): A Graphite.** B and C stay in the switcher for reference only. It's the same neutral family and the same indigo as the approved light theme, so the two themes read as one product. Ink is the most dramatic option. Ember is the warmest, but it drifts away from the light theme.

**Delight, kept honest.** Each flow has one signature moment: the flipping code, the lock clicking shut, ticks landing smallest-first, the arrival seal, and the rewind on undo. None of them delays a task or fakes progress, and all of them go still under reduced motion while the numbers keep updating.

### v2 open questions

1. **Manifest rail.** It's always visible on flow screens at 1100 px. Should it collapse into a button below about 960 px, or always stay?
2. **Nearby discovery.** Showing nearby computers means receivers announce themselves on the LAN (mDNS) while Receive is open. Is that acceptable, or should discovery stay sender-only?
3. **Plugin consent: "Remember".** Should remembering a plugin's permissions be allowed at all in v1 of plugins, or should it always be per move?
4. **Install commands on Done.** These need per-OS install hints in the recipes (`brew`, `apt`, `winget`). Should they be added to the recipe schema?
5. **Can't-move override.** It copies to `~/Carrova/Unplaced/`. Is that the right place?

## Open questions for you

1. **Shell startup files.** The mockups treat Bash and Zsh startup files and Oh My Zsh plugins as "runs code": they start on Leave behind and need a plain-words answer. The current recipes tick them by default. Which should it be?
2. **Who confirms sensitive items?** The mockups ask the sender (one line each, defaulting to leave behind) and the receiver again on the review. Should both sides confirm, or only the receiver?
3. **Terminal UI.** `docs/PLAN.md` doesn't schedule a TUI. Should it be its own phase, or replace the plain CLI prompts?
4. **Font.** Should Carrova bundle Atkinson Hyperlegible (OFL), or use the system UI font? The mockups fall back to the system font.
5. **Pairing code size.** The tiles are 40×58 px with 34 px letters. Is that readable at 2–3 m?
6. **Moving to Windows.** When a Windows computer runs WSL, should Carrova offer to put Bash/Zsh files into WSL (as shown), or always list them as "can't move"?
7. **Accent colour.** The mockups use indigo, `#3346D3` in light and `#A4ADEB` in dark. Keep it, or match a brand colour once the final name is chosen (a Phase 4 decision)?
