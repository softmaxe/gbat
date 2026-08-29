#!/bin/zsh
# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Logitech Mouse Battery
# @raycast.mode compact
# @raycast.packageName GPW Bat
# @raycast.icon 🖱️
# @raycast.description Read the Logitech GPW2 battery level

set -euo pipefail

script_dir="${0:A:h}"
project_dir="${script_dir:h}"

if [[ -n "${GPWBAT_BINARY:-}" ]]; then
  binary_path="$GPWBAT_BINARY"
elif [[ -n "${GPW2_BATTERY_BINARY:-}" ]]; then
  binary_path="$GPW2_BATTERY_BINARY"
elif binary_path="$(command -v gpwbat 2>/dev/null)" && [[ -x "$binary_path" ]]; then
  :
else
  binary_path=""
  for candidate in \
    "/opt/homebrew/bin/gpwbat" \
    "/usr/local/bin/gpwbat" \
    "$HOME/.local/bin/gpwbat" \
    "$project_dir/target/release/gpwbat" \
    "$project_dir/gpwbat"; do
    if [[ -x "$candidate" ]]; then
      binary_path="$candidate"
      break
    fi
  done
fi

if [[ -z "$binary_path" || ! -x "$binary_path" ]]; then
  print -u2 "gpwbat binary not found. Build it with: cargo build --release"
  exit 1
fi

exec "$binary_path"
