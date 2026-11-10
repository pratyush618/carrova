# UI research: a clean, extensible, rewarding Carrova

Input for the mockup redesign (`docs/mockups/`). The verdict on the first round: "too complicated". This note shows what the best transfer, pairing and security products do, and what Carrova takes from them.

## 1. What the best flows do

| Product | What it does well | Carrova takeaway |
|---|---|---|
| **Apple Migration Assistant** | One wizard: pick source, both screens show the same passcode, tick *categories*. A disclosure arrow under each category opens single items. Says plainly that nothing is deleted from the old Mac. [1] | Categories first, items one disclosure away. Say "nothing is removed from this computer". |
| **iPhone Quick Start** | Pairing is one moment: the old phone scans an animated pattern. There's a typed-code fallback ("Authenticate manually"). [2] | The pairing moment can be special. Always offer a plain fallback (our IP-address ticket). |
| **Pixel "Copy apps & data"** | The new phone lists your data with everything on. You turn off what you don't want and tap **Copy**. Pixel 8+ offers **Express** or **Customise**. [3] | Exactly our model: a smart default with one primary button, and *Customize* as the secondary path. |
| **Warp (GNOME, magic-wormhole)** | Sender gets a short word code, receiver types it. That's the whole UI. Encrypted, LAN when possible. [4] | Our code flow is already right. Make the code the star of its own screen and nothing else. |
| **LocalSend** | Three tabs: Send, Receive, Settings. Devices are named automatically. [5] | Home = two verbs. Settings live elsewhere. Computers are named by hostname, never by IP. |
| **Tailscale** | New devices show a "Needs approval" badge and get no access until approved. [6] | One clear badge for "waiting for you". Nothing happens before approval. |
| **1Password Watchtower** | Security problems are a short to-do list on a summary, with details on each item. [7] | Sensitive items are a short list of plain questions, not a red wall. |
| **Linear (2026 refresh)** | "Calmer": parts central to the task stay in focus, while navigation and orientation recede. The sidebar is dimmed. [8] | Hub navigation stays quiet. The wizard hides it completely. |
| **Raycast, Things, Arc** *(observed in use, not cited)* | Fast feedback on every keypress. Single-purpose screens. One delightful flourish (Things' checkbox, Arc's boosts) rather than many. | One signature moment per flow. Instant response to keys. |

## 2. Principles

1. **One primary action per screen.** It's the only filled button, always at the inline end of the footer. Secondary actions are text buttons.
2. **Progressive disclosure, not concealment.** NN/g: defer advanced or rarely used features to a secondary screen, so the product is easier to learn and less error-prone [9]. Secondary-source caution, adopted as a rule: **never hide consequences** (sensitive items, replaced files, overrides) behind disclosure. Hide *controls*, not *facts*.
3. **"User decides everything" survives as visible defaults.** Each summary states the default in words ("3 files will be replaced, backed up first") with a **Change** link that opens the sheet. The full tree, per-file rules, remaps and overrides all live in sheets. Nothing is decided silently.
4. **Plain language.** Use "Your SSH keys" and "Anyone with a copy can log in as you", not "id_ed25519" and "sensitive=true". Paths appear only in sheets.
5. **Trust signals where trust is asked for.** A lock with "Encrypted · only ezio-thinkpad-new can read this" sits on pairing and progress. "Nothing changes until you approve" goes at the top of the review. "Undo for 30 days" goes on done.
6. **Errors give one recovery action.** Say what happened, why, then one button ("Show a new code", "Try the address instead", "Free up space and resume").

## 3. "Addictive" without dark patterns

Carrova is used once or twice a year and handles keys. "Addictive" here means *it feels so good you tell a friend*, not *it pulls you back*.

**We use these:**
- **Signature moments** (one per flow): the split-flap code reveal, the handshake as two computers linking with a lock clicking shut, an "Arrived" stamp on completion.
- **Satisfying progress:** small configs go first (matches the engine's transfer order), so app chips tick off within seconds. A real time estimate shows. Nothing fakes progress.
- **Speed and feedback:** word autocomplete on the code (EFF words have unique 3-letter prefixes), instant validation and a press state on every button.
- **Personality in copy, never in warnings.** "Everything arrived." is warm. Security text is literal.
- **Motion follows WCAG 2.3.3:** every decorative animation has a `prefers-reduced-motion` fallback. Functional feedback (progress numbers, spinner) stays but turns static or subtle [10].

**We reject these** (deceptive-design taxonomy [11][12]):
- **Confirmshaming:** "Leave behind" is never worded as the bad choice.
- **Nagging:** error-report consent is asked once (default off) and never re-prompted. No "rate us".
- **Preselection of risk:** sensitive items always start off.
- **Streaks, badges, counters, notifications to return:** there's no habit loop to build.
- **Fake urgency and fake progress:** timers are real, and "Cancel" is never hidden.
- **Obstruction:** Undo is one click from done and from History, never buried.

## 4. Extensibility: a design system, not screens

New Phase 4–5 screens (plugins, cross-OS, catalogue) are *composed* from a fixed kit:

- **Tokens** (CSS custom properties): colour roles (`--accent`, `--sens`, `--danger`, `--ok`, each with `-soft`), 8-pt spacing `--s-1…--s-8`, type scale 1.25 (13/15/18/22/28/36), radii by hierarchy (control 8, card 14, sheet 20), elevation 0–3, motion durations and easing.
- **Layouts:** (a) *Hub*: quiet top bar plus a page. (b) *Flow*: close button, step dots, a centred 34rem column and a footer action bar. (c) *Sheet*: slides in from the inline end, for any drill-down. (d) *Dialog*: centred, for overrides and confirmations.
- **Components:** button (primary, secondary, quiet, danger), card, choice tile, list row, toggle, checkbox, segmented choice, badge, code tag, progress bar, app chip, banner (info, sens, danger, ok), sheet, dialog, empty/error state, key hint, lock line.
- **Rule:** a new screen picks a layout, then composes components. If a screen needs a new component, add it to the Design system screen first.

## 5. Takeaways mapped to Carrova screens

| Screen | Change |
|---|---|
| Onboarding (3 screens) | One welcome screen: what moves, what doesn't, promise and a consent toggle (off). |
| Home | Two big tiles: **Send** and **Receive**. A quiet "Use an encrypted file instead". The last session is one line with Undo. |
| Send: scan + choose + unrecognised | One **"Here's what we found"** screen: app rows with counts and sizes. A short "Needs your OK" list for keys and startup files, defaulting to leave behind. **Customize** opens the tree sheet. |
| Send: pairing (4 states) | Code tag with split-flap reveal. Fallback address behind "Can't connect?". Wrong code: a new code with one sentence of explanation. |
| Send: their review + sending | Merged into one live screen: "Waiting for them to approve…" turns into progress. |
| Receive: code + connecting | One screen with a handshake animation. |
| Receive: plan | **"Here's what will change"**: three numbers, decisions as a short list, details in a sheet. |
| Receive: transfer + apply | One progress screen. Applying is its final phase. |
| Receive: checklist | A done screen with a stamp, a "Left for you" list and **Undo**. |
| Sessions, recipes, plugins, settings | Hub pages behind a quiet top bar. Phase badges appear only in the dev screen index. |

## 6. Dark theme: before and after

**Before** (first redesign pass, rejected by the user): an inverted palette. It had a blue-tinted near-black (`#15171C` → `#0D0E11` page), a saturated periwinkle accent (`#9AA6FF`), bright status colours (`#FF8A7A`, `#6FD3A0`) and the same drop shadows as light mode. Saturated hues "vibrate" on dark grounds, and shadows don't show on dark, so cards and sheets lost their edges.

**Established practice** (Material dark theme [13], Apple HIG dark mode [14], GitHub Primer [15], Linear [8]):
- Use a near-black neutral base, not `#000`. Material uses `#121212`, Primer `#0D1117`.
- Show elevation with **lighter surfaces**, not shadows. HIG calls these "base" and "elevated" backgrounds, and Material uses tonal overlays.
- Text emphasis is about 87% (high), 60% (medium) and 38% (disabled) white, never pure white.
- Use desaturated, lighter tones for accent and status colours.
- Borders are low-contrast hairlines.

**After:** there's one semantic token table (`surface-0…3`, `text-primary/secondary/muted`, `border`, `border-strong`, `control`, `accent`, `accent-on`, `accent-soft`, `warning`, `danger`, `success`, each `-soft`, `focus`), with light and dark values side by side in `gui.html` and the README.
- The dark base is `#15171C` (neutral, barely cool). Surfaces step up `#1C1E24` → `#23262D` for cards → sheets and dialogs. Wells sink to `#101216`.
- Text is `#E4E6EA` (~90%), `#B0B4BC` (~68%) and `#8E939D` (~55%). Muted text still clears 4.5:1 on the highest surface.
- Borders are ~10% (`#2C2F35`) and ~18% (`#3C4048`) white. Inputs, checkboxes and toggles get a separate `control` edge that meets the 3:1 non-text contrast rule.
- The accent is a desaturated `#A4ADEB` with dark text on it (`#151830`). Status colours are muted: amber `#DDB76A`, red `#EC9C92`, green `#82C9A5`. Their soft fills are barely tinted surfaces.
- Shadows are dropped from cards in dark mode. One soft shadow remains, only for overlays above the scrim.
- The theme follows `prefers-color-scheme` live until the user toggles it manually. The TUI uses the same values for its dark and light terminal palettes.

13. Google, *Material Design: dark theme*: https://m2.material.io/design/color/dark-theme.html
14. Apple, *Human Interface Guidelines: dark mode*: https://developer.apple.com/design/human-interface-guidelines/dark-mode
15. GitHub, *Primer: color*: https://primer.style/foundations/color

(Note: sources 13–15 render client-side and couldn't be re-fetched for this note. The values cited are their long-standing published guidance.)

## Sources

1. Apple, *Transfer your information to Mac*: https://support.apple.com/en-mt/guide/mac-help/mh27921/mac
2. MacRumors, *iPhone to iPhone data migration in iOS 12.4*: https://www.macrumors.com/2019/07/23/iphone-to-iphone-data-migration/ (Apple's own page wasn't retrieved)
3. Google, *Copy apps & data to Pixel*: https://support.google.com/pixelphone/answer/7129955
4. GNOME Apps, *Warp*: https://apps.gnome.org/Warp/
5. Linux Uprising, *LocalSend*: https://linuxuprising.com/2023/03/send-files-securely-over-local-network.html
6. Tailscale, *Device approval*: https://tailscale.com/kb/1099/device-approval
7. 1Password, *Watchtower*: https://support.1password.com/watchtower/
8. Linear, *Behind the latest design refresh*: https://linear.app/blog/behind-the-latest-design-refresh
9. NN/g, *Progressive disclosure*: https://www.nngroup.com/videos/progressive-disclosure/
10. W3C, *Understanding SC 2.3.3 Animation from Interactions*: https://w3c.github.io/wcag/understanding/animation-from-interactions.html
11. Government of Canada, *Types of dark patterns*: https://ised-isde.canada.ca/site/office-consumer-affairs/en/types-dark-patterns
12. Brignull, *Deceptive patterns*: https://www.deceptive.design/
