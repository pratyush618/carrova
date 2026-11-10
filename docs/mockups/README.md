# Carrova UI mockups

These are design mockups, not the real app. They're static, self-contained HTML files with no build step, no network requests and no dependencies.

- **`gui.html`:** the desktop app (Phase 4).
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

## Open questions for you

1. **Shell startup files.** The mockups treat Bash and Zsh startup files and Oh My Zsh plugins as "runs code": they start on Leave behind and need a plain-words answer. The current recipes tick them by default. Which should it be?
2. **Who confirms sensitive items?** The mockups ask the sender (one line each, defaulting to leave behind) and the receiver again on the review. Should both sides confirm, or only the receiver?
3. **Terminal UI.** `docs/PLAN.md` doesn't schedule a TUI. Should it be its own phase, or replace the plain CLI prompts?
4. **Font.** Should Carrova bundle Atkinson Hyperlegible (OFL), or use the system UI font? The mockups fall back to the system font.
5. **Pairing code size.** The tiles are 40×58 px with 34 px letters. Is that readable at 2–3 m?
6. **Moving to Windows.** When a Windows computer runs WSL, should Carrova offer to put Bash/Zsh files into WSL (as shown), or always list them as "can't move"?
7. **Accent colour.** The mockups use indigo, `#3346D3` in light and `#A4ADEB` in dark. Keep it, or match a brand colour once the final name is chosen (a Phase 4 decision)?
