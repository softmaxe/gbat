# README recordings

The terminal animation was captured on September 17, 2026, on macOS 27.0 with a
connected Logitech mouse. Its battery readings are live output.
The Raycast animation is the original GitHub asset and is kept unchanged.

| Asset | Captured version | Source |
| --- | --- | --- |
| [`demo.gif`](../assets/demo.gif) | `gbat 1.0.0` | Local release build of [`v1.0.0`](https://github.com/softmaxe/gbat/tree/v1.0.0), commit `59e7dec96df2e4f5b99920205fa6cb72049193ea` |
| [`raycast-demo.webp`](../assets/raycast-demo.webp) | Original capture; software versions not recorded | Original GitHub asset from commit `59e7dec96df2e4f5b99920205fa6cb72049193ea`, retained unchanged |

The terminal demo shows the binary's own `--version` output and `Battery: 88%`.
A later terminal recording may show a different level or charging state.
The original Raycast animation shows the `Logitech Mouse Battery` command and
`Battery: 86%`. It demonstrates the workflow without identifying a gbat release.

The logo is static artwork. It contains no software version.

## Record the terminal demo

Install Rust and the recording tools, connect the mouse, and keep it awake:

```sh
brew install vhs ffmpeg ttyd
./docs/record-demo.sh
```

The script builds with `--locked`, verifies a real battery read, and records
[`demo.tape`](../demo.tape). The tape puts `target/release` first on `PATH` and
sets `GBAT_BINARY` to that same build, so all four commands use it. There is no
sample-output wrapper.

The recorder exports PNG frames with VHS, encodes them with FFmpeg, and checks
that the GIF can be decoded before replacing `assets/demo.gif`. Temporary files
are removed on exit. This avoids a VHS `0.12.0` issue where the recording context
is cancelled before rendering, leaving the existing GIF untouched despite a
successful exit. See the upstream [render call](https://github.com/charmbracelet/vhs/blob/v0.12.0/evaluator.go#L187).

The checked-in capture used VHS `0.12.0`, ttyd `1.7.7`, and FFmpeg `9.0.1`.
If you change the tape's dimensions, frame rate, padding, theme, or window bar,
update the matching FFmpeg settings in [`record-demo.sh`](record-demo.sh).

## Preserve the original Raycast animation

Keep `assets/raycast-demo.webp` byte-for-byte identical to the original GitHub
asset from commit `59e7dec96df2e4f5b99920205fa6cb72049193ea`. Do not re-record,
re-encode, or replace it during gbat version updates or README drift checks.
Only change it when the user explicitly requests a replacement.

The terminal recorder updates only `assets/demo.gif`. Its release version and
recording date do not apply to the Raycast animation.

## Check before updating the README

- Open the terminal animation and check its version, battery output, and
  readability. A successful recorder exit alone is not enough.
- Verify that the original Raycast animation remains unchanged.
- Check every relative link and image reference in both README files.
- Update the terminal capture's version and tool versions here and its captions
  in both README files together. Keep the binary version tied to `Cargo.toml`
  and the intended release; do not change it to match an older recording.
