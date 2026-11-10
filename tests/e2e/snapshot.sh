#!/usr/bin/env bash
# Prints a canonical listing of $HOME (path, type, mode, sha256 / link target).
# Carrova's own state dir (~/.local/state, and the bare ~/.local dir created as
# its parent) is excluded: it is session state, not user data.
set -euo pipefail
cd "$HOME"
find . -path ./.local/state -prune -o -print0 | sort -z | while IFS= read -r -d '' p; do
  [ "$p" = ./.local ] && continue
  if [ -L "$p" ]; then echo "L $p -> $(readlink "$p")"
  elif [ -d "$p" ]; then echo "D $(stat -c %a "$p") $p"
  else echo "F $(stat -c %a "$p") $(sha256sum < "$p" | cut -c1-64) $p"; fi
done
