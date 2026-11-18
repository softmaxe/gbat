#!/bin/zsh
# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Logitech Mouse Battery
# @raycast.mode compact
# @raycast.packageName G Bat
# @raycast.icon 🖱️
# @raycast.description Read the Logitech GPW2 battery level

set -euo pipefail

if [[ -n "${GBAT_BINARY:-}" ]]; then
  candidates=("$GBAT_BINARY")
else
  candidates=(
    "${commands[gbat]:-}"
    /opt/homebrew/bin/gbat
    /usr/local/bin/gbat
    "$HOME/.local/bin/gbat"
    "${0:A:h:h}/target/release/gbat"
  )
fi

for candidate in "${candidates[@]}"; do
  if [[ -f "$candidate" && -x "$candidate" ]]; then
    [[ "$candidate" == */* ]] || candidate="./$candidate"
    exec "$candidate"
  fi
done

print -u2 "gbat binary not found. Build it with: cargo build --release"
exit 1
