# gbat verification map

This is the maintained map of gbat's user-facing behavior. Read the matching
feature file before driving it. Every listed entry point needs its own outcome;
a terminal script run does not cover the native Raycast entry point.

## Baseline preconditions

- Follow Launch and Doctor in [the skill](../SKILL.md) from a fresh scratch dir.
- Use the absolute release binary built from this checkout with `--locked`.
- Keep `$REPO`, `$BINARY`, `$HARNESS`, `$EVIDENCE`, and `$SCRATCH` in the shell.
- Use macOS or Linux. Raycast and its zsh script are macOS-only product paths.
- For battery journeys, connect a real awake GPW2 via LIGHTSPEED or USB, identify
  the device, and ensure no competing query is using its shared HID interface.
- No fixture can seed hardware state. Missing connection, charging state,
  permission, or Raycast installation fails the dependent journey; record the
  reason and retain the attempt rather than substituting canned output.

## Driving conventions

- Use `capture.py` with a unique evidence subdirectory per action, serially.
- Commands in features assume the Launch variables. On Linux only, replace
  battery `-- "$BINARY"` with `-- sudo -n "$BINARY"` after arranging access.
- Match one stdout battery line and exit status, never README percentages.
- Restore physical states changed during testing; preserve evidence in `target/`.

## Proof and coverage

Each action needs argv, stdout, stderr, exit code, binary identity, and its
result. For battery/charging record the real device preconditions and observed
physical actions too. Native UI proof needs the actual UI invocation/result.
The initial generated-skill smoke run may prove only one feature; name every
unexercised journey as unverified. Successful unit tests do not fill those gaps.

## Features

- [Version identity](version.md): local CLI and installed CLI version reporting.
- [Battery and charging](battery.md): receiver, direct USB, charging suffix.
- [Failure and recovery](failure-recovery.md): offline, absent device,
  permissions, recovery through the same CLI path.
- [Raycast integration](raycast.md): explicit binary, automatic discovery,
  missing executable, native Script Command.
