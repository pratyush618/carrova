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
