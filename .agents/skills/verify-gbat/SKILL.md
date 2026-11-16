---
name: verify-gbat
description: Verify gbat's real macOS/Linux battery CLI and macOS Raycast script with isolated PTY runs, version checks, hardware-aware feature journeys, and retained command evidence. Use after changing CLI output, HID behavior, or Raycast integration.
---

# Verify gbat

Read [the feature map](features/README.md), then the relevant journey. The primary
surface is a one-shot Rust CLI; the secondary surface is the macOS Raycast Script
Command `Logitech Mouse Battery`. No service, port, login, database, migrations,
or seed data is involved. An empty working directory is sufficient for version
checks; successful battery reads require a real awake Logitech mouse.

## Launch

Run from this checkout using Rust/rustup (the version is pinned in
`rust-toolchain.toml`) and `uv`. The helper uses Python 3.12, provisioned by `uv`
if necessary; Python is a verification dependency, not an app dependency.
On Linux the native hidraw backend needs a C compiler/linker, not `libudev-dev`
or `libusb`. Build as your regular user.

Keep these variables in one shell across launch, doctor, drive, and cleanup:

```sh
REPO="$(git rev-parse --show-toplevel)"
cd "$REPO"
CARGO_TARGET_DIR="$REPO/target" cargo build --release --locked
BINARY="$REPO/target/release/gbat"
HARNESS="$REPO/.agents/skills/verify-gbat/scripts/capture.py"
mkdir -p "$REPO/target/verification-evidence"
EVIDENCE="$(mktemp -d "$REPO/target/verification-evidence/run.XXXXXX")"
SCRATCH="$(mktemp -d "${TMPDIR:-/tmp}/gbat-verify.XXXXXX")"
printf 'Evidence: %s\nScratch: %s\n' "$EVIDENCE" "$SCRATCH"
trap 'rmdir -- "$SCRATCH"' EXIT
```

The build must succeed before driving. A successful doctor below establishes
CLI readiness and the manifest version. It does not establish hardware
readiness. There is no persistent app process: start each action with the helper
in its own PTY, which exits and is reaped before the next action. Teardown is
the Cleanup section; run it after failures too.

Hardware queries cannot be isolated by working directory: interfaces are shared,
requests use one software ID, and input reports are flushed. Never run two
battery queries concurrently against the same receiver, including Raycast
refreshes or another agent's run. If another driver is active, defer the query;
do not kill or commandeer it. Version-only runs can coexist. There is no device
selector: multiple Logitech devices can make the reading belong to another
device. Establish the intended device/connection before claiming GPW2 proof.

## Doctor

This is the read-only instance check. It invokes the exact local binary without
opening HID, requires the version in `Cargo.toml`, and records its SHA-256:

```sh
uv run --no-project --python 3.12 "$HARNESS" \
  --evidence "$EVIDENCE/doctor" --cwd "$SCRATCH" --binary "$BINARY" \
  --feature version-local --expect version -- "$BINARY" --version
```

Require helper exit `0`, `result.json` `passed: true`, exact version stdout,
and empty stderr. The helper records a binary hash on every action: compare it
with the doctor hash to detect a build replaced during verification. Version
alone cannot establish freshness, so always build this checkout first. If
anything looks off, repeat this check in a new evidence directory, retaining
the failed attempt. On Linux this command needs no `sudo`.

## Drive

The executable [PTY helper](scripts/capture.py) accepts exact commands after
`--`; it never runs them through a shell. Every invocation requires a new
evidence subdirectory and an existing, disposable scratch directory. Stdout and
stdin use a private controlling PTY; stderr is redirected to its own proof file
so channel assertions remain possible. It supplies no interactive input. The
default deadline is 30 seconds, not the internal six-second discovery budget;
use `--timeout 60` only for a justified slow host.

For a real battery read on macOS:

```sh
uv run --no-project --python 3.12 "$HARNESS" \
  --evidence "$EVIDENCE/battery-terminal" --cwd "$SCRATCH" --binary "$BINARY" \
  --feature battery-terminal --expect battery -- "$BINARY"
```

For the corresponding Linux action, use `-- sudo -n "$BINARY"` instead.
Build first as a regular user. If `sudo -n` needs authentication, use the
environment's secret-delivery mechanism; do not put passwords into commands,
artifacts, or this skill. A sudo error is a failed prerequisite, not a gbat
failure-path pass. Do not change udev rules or device permissions for a run.

Follow the map for charging, offline/recovery, and Raycast actions. A missing
mouse or required charging state is a failed journey with its unmet precondition,
not a successful substitute. Only the first CLI argument exactly equal to
`--version` avoids HID. There is no `--help`, dry-run, or device-list mode;
unknown arguments execute a battery query. The Raycast script does not forward
arguments, so `mouse-battery.sh --version` also queries hardware.

## Evidence

Keep proof in `$REPO/target/verification-evidence/run.XXXXXX`, outside scratch.
`target/` is already ignored by Git. Copy evidence elsewhere before `cargo clean`
or deleting the checkout. Each action contains:

- `action.json`: feature/entry-point ID, exact argv, working directory, UTC time,
  platform, binary path/hash, manifest version, expected behavior, scratch before.
- `terminal.bin` and `events.jsonl`: raw stdout PTY bytes and timed output chunks.
- `stdout.txt` and `stderr.txt`: stdout with terminal CRLF normalized to LF and
  separately captured stderr.
- `result.json`: actual app exit code, timeout/interruption/capture error,
  assertion result, scratch after,
  and whether scratch files stayed unchanged. Helper exit `0` means all its
  checks passed; app exit code is recorded separately.

Record connection mode, intended device identity, wake/charging conditions,
manual actions, and feature outcomes in `$EVIDENCE/notes.md`. Capture the action
and resulting state through the real user entry point. For hardware queries,
compare charging suffix with independently observed cable/charging conditions;
do not require an exact historical battery percentage. A command that merely
prints a plausible battery line cannot prove device identity or charging state.

Observe relevant side effects alongside output: the helper compares scratch
file contents; it does not audit the whole filesystem or network. gbat has no
documented state files, network calls, or browser launches, but battery queries
send real HID++ writes before reading replies. Do not call them pure device
reads. `--version` returns before HID initialization. No mock battery provider,
replacement executable, or internal test setter proves real hardware behavior.
Existing protocol tests are useful supplementary checks, not CLI/hardware proof.

For Raycast UI proof, retain the actual invocation and visible compact result
using available native-app capture tooling; terminal execution of its script is
a separate path. Do not reuse the historical README GIF/WebP as fresh evidence.
Do not run `docs/record-demo.sh` for routine verification: it replaces
`assets/demo.gif`.

## Cleanup

The helper waits for its child and closes its PTY. On timeout, SIGINT, or SIGTERM
it kills only the process group it created and reaps the child. Never kill by
process name. No server remains to stop. Close only UI windows opened by the
run and restore any device state changed specifically for a journey.

After every attempt, including a failed one, remove only that run's scratch:

```sh
rmdir -- "$SCRATCH"
trap - EXIT
test ! -e "$SCRATCH"
test -s "$EVIDENCE/doctor/action.json"
test -s "$EVIDENCE/doctor/result.json"
test -s "$EVIDENCE/doctor/stdout.txt"
printf 'Retained proof: %s\n' "$EVIDENCE"
```

Also confirm the driven feature's `action.json`, `result.json`, stdout/stderr,
and terminal evidence survive. Never remove `$EVIDENCE` in cleanup. For a failed
build before scratch exists, no instance was launched and there is nothing to
terminate. Scratch should be empty: if `rmdir` fails, inspect it and remove only
the run-created files before retrying; do not recursively delete an uninspected
path. Do not erase failed evidence when retrying. SIGKILL or machine termination
cannot be handled; such runs may retain partial evidence and need inspection.

## Helpers

All invocations above run `scripts/capture.py` through `uv`; it is also directly
executable. Inspect its options without touching HID with:

```sh
uv run --no-project --python 3.12 "$HARNESS" --help
```

`--expect version` requires the current manifest version; `battery` requires one
valid `Battery: N%` line with an optional charging suffix; `error` requires app
exit `1`, empty stdout, and a known gbat error prefix; `missing-binary` requires
the Raycast wrapper's exact missing-executable message. Every expectation also
requires no timeout and unchanged scratch file contents. `error` recognizes a
family of failures: read stderr to assert the specific scenario in the map.
The helper cannot certify hardware setup or native Raycast UI behavior.
