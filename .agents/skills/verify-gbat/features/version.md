# Version identity

Users can identify the local or installed gbat version without connecting a
mouse, granting HID access, or elevating privileges.

## Sub-features

- `version-local` reports the release build version from this checkout.
- `version-installed` reports the executable selected by the user's `PATH`.

## How to get to it (user POV)

- Build from source and run `./target/release/gbat --version`.
- After installing gbat, run `gbat --version` in Terminal or a Linux shell.

## Driving it with capture.py

Preconditions:

- Launch has built the release binary and created the shell variables.
- For the installed entry point, a real gbat installation is on `PATH` and its
  expected release version is known. Do not change the installation for a smoke test.

- **Doctor/local entry.** Run the Doctor command in the skill. Expect exactly
  `gbat ` plus `Cargo.toml`'s version and a newline, empty stderr, app exit `0`,
  helper exit `0`, and unchanged empty scratch. Its `doctor/` artifacts prove
  `version-local`; no second identical invocation is needed.
- **Installed entry.** Resolve `INSTALLED="$(command -v gbat)"`, require an
  absolute executable path, then run
  `uv run --no-project --python 3.12 "$HARNESS" --evidence "$EVIDENCE/version-installed" --cwd "$SCRATCH" --binary "$INSTALLED" --feature version-installed --expect version -- "$INSTALLED" --version`.
  For an installation of this checkout's version, require the same stdout and
  status as local. If the installed release differs, this expectation fails:
  record its observed version and coverage mismatch rather than claiming the
  checkout was verified through that installed binary.
- **Proof.** Inspect `action.json` for the absolute binary/hash and `result.json`
  for `passed: true`. Run Cleanup and confirm both files and `stdout.txt` remain.

## Gotchas

- Only `--version` as the first argument avoids hardware. `--help` is not help.
- A matching version is not proof that two binaries contain the same code; use
  recorded SHA-256 and build this checkout immediately before verification.
- If no installed executable exists, `version-installed` fails its precondition;
  the local entry remains independently verifiable.
