#!/bin/zsh
# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Logitech Mouse Battery
# @raycast.mode compact
# @raycast.packageName G Bat
# @raycast.icon 🖱️
# @raycast.description Read the Logitech GPW2 battery level

set -euo pipefail

script_dir="${0:A:h}"
project_dir="${script_dir:h}"

if [[ -n "${GBAT_BINARY:-}" ]]; then
  binary_path="$GBAT_BINARY"
elif [[ -n "${GPWBAT_BINARY:-}" ]]; then
  binary_path="$GPWBAT_BINARY"
elif [[ -n "${GPW2_BATTERY_BINARY:-}" ]]; then
  binary_path="$GPW2_BATTERY_BINARY"
elif binary_path="$(command -v gbat 2>/dev/null)" && [[ -x "$binary_path" ]]; then
  :
else
  binary_path=""
  for candidate in \
    "/opt/homebrew/bin/gbat" \
    "/usr/local/bin/gbat" \
    "$HOME/.local/bin/gbat" \
    "$project_dir/target/release/gbat" \
    "$project_dir/gbat"; do
    if [[ -x "$candidate" ]]; then
      binary_path="$candidate"
      break
    fi
  done
fi

if [[ -z "$binary_path" || ! -x "$binary_path" ]]; then
  print -u2 "gbat binary not found. Build it with: cargo build --release"
  exit 1
fi

exec "$binary_path"
