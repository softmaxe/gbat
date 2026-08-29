#!/bin/zsh
# Required parameters:
# @raycast.schemaVersion 1
# @raycast.title Logitech Mouse Battery
# @raycast.mode compact
# @raycast.packageName GPW2 Battery
# @raycast.icon 🖱️
# @raycast.description Read the Logitech GPW2 battery level

set -euo pipefail

script_dir="${0:A:h}"
project_dir="${script_dir:h}"

if [[ -n "${GPW2_BATTERY_BINARY:-}" ]]; then
  binary_path="$GPW2_BATTERY_BINARY"
else
  binary_path=""
  for candidate in \
    "$project_dir/target/release/gpw2-battery" \
    "$project_dir/gpw2-battery" \
    "$HOME/.local/bin/gpw2-battery" \
    "/opt/homebrew/bin/gpw2-battery" \
    "/usr/local/bin/gpw2-battery"; do
    if [[ -x "$candidate" ]]; then
      binary_path="$candidate"
      break
    fi
  done
fi

if [[ -z "$binary_path" || ! -x "$binary_path" ]]; then
  print -u2 "gpw2-battery binary not found. Build it with: cargo build --release"
  exit 1
fi

exec "$binary_path"
