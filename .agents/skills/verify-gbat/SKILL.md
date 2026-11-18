---
name: verify-gbat
description: Verify gbat's real macOS/Linux battery CLI and macOS Raycast script with isolated PTY runs and retained evidence. Use after changing CLI output, HID behavior, or Raycast integration.
---

# Verify gbat

Choose the journey that covers the change:

- [Version identity](features/version.md) covers the checkout and installed executables.
- [Battery and charging](features/battery.md) covers receiver, direct USB, and charging state.
- [Failure and recovery](features/failure-recovery.md) covers absent devices, offline mice, Linux permissions, and recovery.
- [Raycast integration](features/raycast.md) covers binary selection, missing executables, and the native Script Command.

## Launch

Build as the regular user with the pinned Rust toolchain. The [PTY helper](scripts/capture.py)
uses Python 3.12 through `uv`. Keep these variables in one shell:

```sh
REPO="$(git rev-parse --show-toplevel)"
cd "$REPO"
CARGO_TARGET_DIR="$REPO/target" cargo build --release --locked
BINARY="$REPO/target/release/gbat"
HARNESS="$REPO/.agents/skills/verify-gbat/scripts/capture.py"
mkdir -p "$REPO/target/verification-evidence"
EVIDENCE="$(mktemp -d "$REPO/target/verification-evidence/run.XXXXXX")"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/gbat-verify.XXXXXX")"
trap 'rmdir -- "$SCRATCH"' EXIT
```

## Doctor

After a successful build, verify the exact executable against the manifest.
Only `--version` as the first CLI argument skips HID initialization.

```sh
uv run --no-project --python 3.12 "$HARNESS" \
  --evidence "$EVIDENCE/doctor" --cwd "$SCRATCH" --binary "$BINARY" \
  --feature version-local --expect version -- "$BINARY" --version
```

Require helper exit `0` and `passed: true` in `result.json`. Compare the binary
SHA-256 in subsequent actions with the doctor's hash for the same executable.

## Drive

Run each relevant journey with a fresh evidence subdirectory per action. The
helper launches the argv after `--` in a private PTY, captures stderr separately,
and enforces a 30-second deadline by default. To inspect all options, run
`uv run --no-project --python 3.12 "$HARNESS" --help`.

For successful battery queries, identify the intended awake GPW2 and connection.
The CLI can select any responding Logitech device. Serialize queries on the same
receiver, including Raycast refreshes. They send HID++ requests using a shared
software ID and flush input reports. On Linux, use `-- sudo -n "$BINARY"` in
battery examples when elevated device access is needed. Handle authentication
outside the capture. Keep device permissions unchanged.

`--expect battery` checks the percentage and allows an optional charging suffix.
Check the suffix against the observed charging state. `--expect error` checks a
family of CLI errors. Inspect stderr for the specific failure in the selected
journey. Record unavailable hardware or permissions as unmet preconditions.
Native Raycast verification requires the visible Script Command invocation and
result. The terminal wrapper ignores arguments, including `--version`.

## Evidence

Keep evidence under the ignored `target/verification-evidence/` directory.
Use a new directory for retries so earlier outcomes remain available.

- `action.json` records argv, working directory, timestamp, platform, binary path
  and hash, manifest version, expectation, and the initial scratch snapshot.
- `terminal.bin` and `events.jsonl` hold raw PTY output and timed chunks.
  `stdout.txt` normalizes terminal CRLF to LF. `stderr.txt` captures stderr.
- `result.json` records the child exit code, timeout, interruption, capture
  errors, assertion, and final scratch snapshot. A pass requires the expected
  exit and output, a completed capture, and unchanged scratch entries.
- Record the device identity, connection, wake/charging conditions, manual
  actions, and feature outcomes in `$EVIDENCE/notes.md`.

## Cleanup

The helper reaps its child when it exits. After successes and failures, restore
device state changed for the journey, close windows opened for the run, and
remove the empty scratch directory:

```sh
rmdir -- "$SCRATCH" && trap - EXIT
```

If scratch contains unexpected entries, inspect them against the captured
snapshot before removal. Retain the action and result files, including failed
runs. Copy evidence elsewhere before cleaning the Cargo target directory.
