# Failure and recovery

When no battery status can be read, gbat reports an actionable error instead of
inventing a percentage. Users can restore connection or wake the mouse and retry.

## Sub-features

- `failure-absent` reports no responsive interface.
- `failure-offline` reports the receiver has no connected mouse.
- `failure-permission` explains a real Linux HID access denial.
- `failure-recovery` reads a real battery after the failed condition is resolved.

## How to get to it (user POV)

- Run `gbat` without a responsive Logitech device, or with the mouse switched off.
- On Linux, run without `sudo` when the device actually requires elevated access.
- Restore connection, turn on or move the mouse, then run the query again.

## Driving it with capture.py

Preconditions:

- Doctor passes. Establish the specific physical/error condition independently.
- Do not disconnect the user's active input devices or change their permissions
  just to manufacture an error without scope to do so. If the required state
  is unavailable, report that journey failed its precondition.

- **Absent device.** In an established no-responsive-device state, run
  `uv run --no-project --python 3.12 "$HARNESS" --evidence "$EVIDENCE/failure-absent" --cwd "$SCRATCH" --binary "$BINARY" --feature failure-absent --expect error -- "$BINARY"`.
  Require app exit `1`, empty stdout, and stderr beginning
  `No responsive Logitech HID++ interface found.`. `--expect error` also accepts
  other gbat errors, so inspect this exact prefix to prove this scenario.
- **Offline receiver.** With the receiver attached and its mouse switched off,
  run the same recipe with `failure-offline` as directory/feature. Require stderr
  beginning `The receiver reports no connected mouse.` and app exit `1`.
- **Linux permissions.** On a Linux host where unprivileged device access is
  denied, run the binary without sudo in directory/feature `failure-permission`.
  Require `Permission denied (os error 13)` and the Linux sudo guidance in the
  gbat error, empty stdout and app exit `1`. Do not use a sudo authentication
  error as evidence of this feature, or revoke working permissions to force it.
- **Recover through the same entry.** Restore the attempted connection/wake state
  or use Linux `sudo -n`. Run
  `uv run --no-project --python 3.12 "$HARNESS" --evidence "$EVIDENCE/failure-recovery" --cwd "$SCRATCH" --binary "$BINARY" --feature failure-recovery --expect battery -- "$BINARY"`
  (Linux: `-- sudo -n "$BINARY"`). Require valid battery stdout, empty stderr
  and exit `0`. Record the recovery action between the failed/successful runs.
- **Proof.** Keep both failed and recovered artifacts, physical conditions and
  recovery action. An error-only run does not prove recovery. Run Cleanup.

## Gotchas

- No hardware can yield an absent-device error, but cannot prove offline,
  permissions, battery success, or recovery without the corresponding setup.
- A receiver may answer differently depending on pairing/wake state. Do not
  relabel a different error to make the offline expectation pass.
- Initialization/HID errors are possible; retain them and diagnose the actual
  failure. Never accept exit `1` alone as the intended failure scenario.
