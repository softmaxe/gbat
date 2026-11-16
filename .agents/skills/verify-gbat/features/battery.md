# Battery and charging

Users run gbat once to see the connected mouse's current battery percentage and
whether it is actively charging, through a LIGHTSPEED receiver or direct USB.

## Sub-features

- `battery-receiver` reads an awake mouse through LIGHTSPEED.
- `battery-usb` reads a mouse connected directly through USB.
- `battery-charging` shows `(charging)` during active charging.
- `battery-idle` omits the suffix when not actively charging.

## How to get to it (user POV)

- Connect the LIGHTSPEED receiver, wake the mouse, and run `gbat` in Terminal.
- Connect the mouse by USB and run the same command.
- On Linux, run `sudo /absolute/path/to/gbat` for either connection.

## Driving it with capture.py

Preconditions:

- Doctor passes; a real intended GPW2 is connected and awake, with no competing
  HID driver/query. Record connection and other connected Logitech devices.
- Establish active charging and non-charging separately to cover both suffix
  states. Physical access is required; absence of either state fails that case.
- On Linux, the command has permission to open hidraw for reading and writing.

- **Receiver entry.** With the receiver connected and the mouse unplugged from
  USB, record that setup in `notes.md`. Run
  `uv run --no-project --python 3.12 "$HARNESS" --evidence "$EVIDENCE/battery-receiver" --cwd "$SCRATCH" --binary "$BINARY" --feature battery-receiver --expect battery -- "$BINARY"`.
  Expect one `Battery: N%` line, `N` within `0..100`, empty stderr, app exit `0`,
  no charging suffix, and unchanged scratch.
- **Direct USB entry.** With the mouse connected by USB, ensure the receiver
  cannot provide an ambiguous second path; change connections only when the
  current verification scope permits it. Run the same command with evidence
  `"$EVIDENCE/battery-usb"` and feature `battery-usb`. Require valid battery
  output and record the independently observed charging state.
- **Charging state.** During active charging, repeat in a new directory
  `"$EVIDENCE/battery-charging"` with feature `battery-charging`. Require the
  exact ` (charging)` suffix in stdout, not merely a successful `battery`
  assertion. After charging stops or USB is unplugged, repeat as `battery-idle`
  and require the suffix to be absent. Record the actions and both results.
- **Proof.** Retain the command/output/status artifacts for each reached entry,
  connection and charging observations in `notes.md`, and unchanged scratch.
  Restore connections changed solely for testing, then run Cleanup.

## Gotchas

- A receiver-only pass does not prove direct USB, and vice versa.
- The CLI cannot select a device; another Logitech device may answer first.
- An idle/offline mouse is an unmet success precondition, not valid battery proof.
- At `100%` a mouse may stop charging despite being plugged in. That setup
  cannot prove `battery-charging`; record the missing active-charging state.
- Values can change between reads; the historical README percentages are not fixtures.
