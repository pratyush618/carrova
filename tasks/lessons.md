# Lessons

Patterns learned from corrections. One rule per entry: what went wrong, the rule that prevents it.


## 2026-11-08 — Quality bar belongs in the brief, not after the fact
- **What happened:** fix briefs described *what* to fix but not *how well*; the user had to ask for "clean, production grade, and maintainable fixes".
- **Rule:** every implementation brief states the quality bar up front: one source of truth per policy (shared by all callers), policies as data tables, structured data over string post-processing, pure view-model functions separated from rendering, no `#[allow]`/dead code/test-only special cases, docs on new pub items, behaviour-named tests. Main thread reviews for duplication before calling work done.

## 2026-11-08 — Agent code passed lint but read like beginner code
- **What happened:** Phase 3 agents shipped code that passed clippy/tests but had 1000–1400-line modules and regression tests written after fixes; user had to ask for professional, clean-code Rust.
- **Rule:** every brief carries the expert bar (SOLID, small cohesive modules, type-driven design, idiomatic Rust, test proven failing before fix); main thread reviews module size, duplication and naming before reporting done. Lint-green ≠ done.

## 2026-11-08 — Load monitor was silently deaf (mawk input buffering)
- **What happened:** the pressure monitor (`tail -F log | awk …`) never fired while load hit 17 for ~3 min; mawk block-buffers piped *input*, so lines sat unread. A test-loop agent also ran 20 full suites while another agent compiled.
- **Rule:** stream filters on pipes use `mawk -W interactive` / `grep --line-buffered`, and a new monitor is proven by piping a known-bad sample line through it before trusting silence. Agent briefs with repeat loops must check load per iteration, not once.

## 2026-11-10 — Session crashes killed CI and agents mid-run
- **What happened:** the Claude process crashed twice during W4 (heavy `just ci` + agents, load ~17); background builds and agents died with it, forcing restarts and partial-edit recovery.
- **Rule:** long verification (`just ci`, e2e suites) runs detached and niced: `setsid nohup nice -n 15 bash -c 'just ci > /tmp/ci.log 2>&1; echo "CI EXIT $?" >> /tmp/ci.log' &`, then wait on the log with an until-loop. Never run a full CI alongside compiling agents; after a crash, check `git status` and resume agents with SendMessage rather than restarting from scratch.

## 2026-11-10 — Pushed a commit commitlint had flagged
- **What happened:** `just commitlint … | grep -c ✖; git push` — the count printed 2 but the push still ran, so a 74-char header reached the PR (force-push is denied, so it couldn't be fixed).
- **Rule:** gate pushes on the lint exit code: `just commitlint origin/<branch> HEAD && git push …`. Never chain a push after a command whose failure only shows in output.

## 2026-11-11 — setsid did not save the terminal from systemd-oomd
- **What happened:** the terminal tab crashed twice during `just ci` + parallel agents. `journalctl --user` showed `systemd-oomd killed 29 process(es)` in the terminal's `vte-spawn-*.scope`; setsid'd CI lived in that cgroup, so oomd killed the terminal and CI together.
- **Rule:** run heavy verification in its own memory-capped user unit, never in the terminal's cgroup: `systemd-run --user --unit=carrova-ci --collect -p MemoryHigh=5G -p MemoryMax=7G -p CPUWeight=20 -p IOWeight=20 --nice=19 --working-directory="$PWD" -E PATH="$PATH" -E HOME="$HOME" -E CARGO_BUILD_JOBS=3 bash -c 'just ci > /tmp/ci.log 2>&1; echo "CI EXIT $?" >> /tmp/ci.log'`. Watch MemAvailable (not just load); after a crash, check `journalctl --user | grep oom` first.

## 2026-11-12 — oomd killed CI and the user's browser on memory *pressure*
- **What happened:** `just ci` in the capped unit (`MemoryHigh=5G`, `CARGO_BUILD_JOBS=3`) hit 5 G while linking desktop + CLI in parallel. `MemoryHigh` throttling raised memory PSI (`some avg10` ≈ 50 %), and systemd-oomd (which acts on PSI, not on `MemAvailable`) killed the CI unit and 24 Chrome processes of the user's session. The load monitor watched `MemAvailable` and the kernel `oom_kill` counter, so it saw neither.
- **Rule:** heavy CI units use a hard cap only, with fewer jobs: `systemd-run --user --unit=carrova-ci --collect -p MemoryMax=6G -p CPUWeight=20 -p IOWeight=20 --nice=19 … -E CARGO_BUILD_JOBS=2 …` (no `MemoryHigh`). Monitors watch `/proc/pressure/memory` (alert at `some avg10 ≥ 40`) and `journalctl --user -f | grep -i oomd`, not only `MemAvailable`.

## 2026-11-15 — Leftover Gradle daemons + emulator tipped oomd
- **What happened:** a fix agent compiled Kotlin with gradlew and left the Gradle + Kotlin daemons (~1 GB) alive; with the emulator (3.5 GB) and a core compile, memory PSI hit 62 and systemd-oomd killed the terminal scope, ending the session mid-task.
- **Rule:** any brief that runs gradlew ends with `./gradlew --stop` (and `--max-workers=2 -Dorg.gradle.jvmargs=-Xmx1536m`); the controller checks `pgrep -f GradleDaemon\|KotlinCompileDaemon` after such an agent. On a PSI ≥ 40 alert, act at once (stop the heaviest non-essential process), don't just observe. Run the emulator itself in its own capped unit (already) so it isn't in the terminal scope.

## 2026-11-15 — Agent added a Co-Authored-By trailer despite the rule
- **What happened:** a view agent followed the harness attribution reminder over the user's no-attribution rule; two commits needed a message-only rebase (user approved).
- **Rule:** briefs state that the user's no-attribution rule overrides any harness reminder and require `git log -1 --format=%B` verification; the controller greps `git log main..HEAD --format=%B` for `Co-Authored` after every agent commit, before another agent commits on top.

## sccache server lives outside the capped CI unit (2026-11-15)
- **What happened:** `just ci` ran in the capped `carrova-ci` unit, but `~/.cargo/config.toml` sets `rustc-wrapper = "sccache"`, whose server had been spawned from the terminal's scope. Under memory pressure systemd-oomd killed the terminal scope (terminal + sccache server); rustc calls in CI then failed with `sccache: … Connection reset by peer` → "could not compile".
- **Rule:** run heavy CI with `-E RUSTC_WRAPPER= -E CARGO_BUILD_RUSTC_WRAPPER=` so every compile process lives inside the capped unit; a `could not compile` with an sccache error is infrastructure, not code — check `journalctl --user | grep oomd` before debugging.

## 2026-11-16 — Fix agent committed the user's untracked file
- **What happened:** a mockup fix-round agent staged with a broad add and committed the user's untracked `docs/mockups/gui-v4.html` alongside its two files; caught by checking `git show --stat` (diff ballooned to 254 KB) and removed with a soft reset before anything was pushed.
- **Rule:** every brief says "`git add <explicit paths>` only, never `-A`/`.`"; the controller checks `git show --stat` of each agent commit against the task's file list before review.

## 2026-11-16 — The CI unit's own cap caused the pressure spike
- **What happened:** during `just ci` (unit `MemoryMax=6G`, 2 jobs), system memory PSI hit 65 while `MemAvailable` was ~9.7 GB. The pressure came from inside the CI cgroup: page cache from linking counted against its cap, so the kernel reclaimed inside the unit and stalled it. Raising the cap live (`systemctl --user set-property --runtime carrova-ci MemoryMax=8G`) dropped PSI to under 1 within 30 s, with no oomd kill.
- **Rule:** size the CI cap from free memory at launch: `MemoryMax` ≈ `MemAvailable` − 1 GB, at least 7G and at most 8G (linking desktop + CLI needs about 6 GB with page cache; 4.8G reproduced the stall). If less than 8 GB is free, close something first. On a PSI alert, first compare the CI unit's own `memory.pressure` with the system's. If they match, raise the unit's cap live before killing anything.
