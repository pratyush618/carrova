#!/usr/bin/env bash
# Fails if test code lives under a `src/` dir in crates/ or apps/.
# Layout rule (CLAUDE.md, Testing): tests live in `tests/<module>/`;
# Rust unit tests are wired from src with `#[path = ".../tests/<module>/unit.rs"]`.
set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")/.."

violations=0

report() {
    printf '%s: %s\n' "$1" "$2" >&2
    violations=$((violations + 1))
}

while IFS= read -r -d '' src; do
    while IFS= read -r -d '' file; do
        report "$file" "inline \`mod tests {\` block"
    done < <(grep -rlZ --include='*.rs' -E '^\s*(pub(\([a-z]+\))?\s+)?mod tests\s*\{' "$src" || true)

    while IFS= read -r -d '' file; do
        report "$file" "test file under src/"
    done < <(find "$src" -type f \( -name 'tests.rs' -o -name '*_tests.rs' -o -name '*_test.rs' \) -print0)

    while IFS= read -r -d '' dir; do
        report "$dir" "tests/ directory under src/"
    done < <(find "$src" -mindepth 1 -type d -name tests -print0)
done < <(find crates apps \( -name node_modules -o -name target \) -prune \
    -o -type d -name src -prune -print0)

if [ -d apps/desktop/src ]; then
    while IFS= read -r -d '' file; do
        report "$file" "front-end test file under src/ (move to apps/desktop/tests/<module>/)"
    done < <(find apps/desktop/src -type f \( -name '*.test.ts' -o -name '*.test.tsx' \) -print0)
fi

if [ "$violations" -gt 0 ]; then
    printf 'check-test-layout: %d violation(s); tests belong in tests/<module>/\n' "$violations" >&2
    exit 1
fi
echo "check-test-layout: ok"
