# Raycast integration

On macOS, users run `Logitech Mouse Battery` in Raycast to see the real CLI's
battery line. Its script can also be run directly in Terminal.

## Sub-features

- `raycast-explicit` uses the executable selected by `GBAT_BINARY`.
- `raycast-discovery` finds gbat through the documented automatic search order.
- `raycast-missing` reports an explicitly configured non-executable path.
- `raycast-ui` shows the real Script Command's result in Raycast.

## How to get to it (user POV)

- Run `GBAT_BINARY=/absolute/path/to/gbat ./raycast/mouse-battery.sh` in Terminal.
- Run the script without the override to use its binary discovery.
- Add the script directory to Raycast Settings, search `Logitech Mouse Battery`,
  and execute it. Homebrew installs the CLI only; script registration is separate.

## Driving it with capture.py

Preconditions:

- macOS with zsh; Doctor passes. Hardware-success cases also need an awake mouse.
- The native UI case requires Raycast installed and the script directory already
  registered, or authorization to register it. Record this prerequisite.
- Check `test -x "$REPO/raycast/mouse-battery.sh"` for the executable script.

- **Explicit terminal entry.** Run
  `uv run --no-project --python 3.12 "$HARNESS" --evidence "$EVIDENCE/raycast-explicit" --cwd "$SCRATCH" --binary "$BINARY" --feature raycast-explicit --expect battery -- env GBAT_BINARY="$BINARY" "$REPO/raycast/mouse-battery.sh"`.
  Require valid battery stdout, empty stderr and app exit `0`.
- **Automatic discovery entry.** Run
  `uv run --no-project --python 3.12 "$HARNESS" --evidence "$EVIDENCE/raycast-discovery" --cwd "$SCRATCH" --binary "$BINARY" --feature raycast-discovery --expect battery -- env -u GBAT_BINARY PATH="$REPO/target/release:$PATH" "$REPO/raycast/mouse-battery.sh"`.
  This exercises the PATH entry with this checkout's build. When modifying
  fallback discovery, exercise each affected location in the
  [documented order](../../../../README.md#raycast) on an appropriately
  configured host. Preserve the user's installed binaries.
- **Missing configured executable.** Confirm `test ! -e "$SCRATCH/no-gbat"`.
  Run
  `uv run --no-project --python 3.12 "$HARNESS" --evidence "$EVIDENCE/raycast-missing" --cwd "$SCRATCH" --binary "$BINARY" --feature raycast-missing --expect missing-binary -- env GBAT_BINARY="$SCRATCH/no-gbat" "$REPO/raycast/mouse-battery.sh"`.
  Require app exit `1`, empty stdout and exact stderr
  `gbat binary not found. Build it with: cargo build --release` plus a newline.
  Repeat with `GBAT_BINARY="$SCRATCH"` and `raycast-directory` as the evidence
  directory and feature. A searchable directory also has its execute bit set.
  Both cases should reach the same missing-executable diagnostic even when
  a working gbat is on PATH.
- **Native UI entry.** Using available native-app control, open Raycast, search
  the exact command `Logitech Mouse Battery`, and execute it. Capture the action
  and visible compact battery result in `$EVIDENCE/raycast-ui/`, with connection
  conditions and the configured executable identity in `notes.md`. Terminal
  `GBAT_BINARY` does not propagate into an already running Raycast. If the UI
  cannot be driven or its configured binary cannot be identified, record the
  unmet precondition; a terminal script pass does not prove this entry.
- **Proof.** Retain each terminal action/result and native UI evidence separately.
  Close only windows opened for verification, restore any authorized registration
  or script configuration changes, and run Cleanup.

## Gotchas

- The script ignores its arguments, including `--version`; it queries hardware.
- Invalid `GBAT_BINARY` takes precedence even when a working gbat is on PATH.
