#!/usr/bin/env bash
# PostToolUse: format edited Rust files. Never blocks; formatting errors are ignored
# (compile errors surface through `just lint` instead).
set -euo pipefail

file=$(jq -r '.tool_input.file_path // empty')
[[ "$file" == *.rs && -f "$file" ]] || exit 0

rustfmt --edition 2024 "$file" >/dev/null 2>&1 || true
exit 0
