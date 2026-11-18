# README recordings

| Asset | Capture | Visible output |
| --- | --- | --- |
| [`demo.gif`](../assets/demo.gif) | Local release build on macOS 27.0.1, October 13, 2026, with a connected Logitech mouse | `gbat 0.0.1` and `Battery: 61%` |
| [`raycast-demo.webp`](../assets/raycast-demo.webp) | Original GitHub asset. Software versions were not recorded. | `Logitech Mouse Battery` and `Battery: 86%` |

## Record the terminal demo

On macOS, install Rust and the recording tools, connect the mouse, and keep it awake:

```sh
brew install vhs ffmpeg ttyd
./docs/record-demo.sh
```

The script builds with `--locked`, verifies a real battery read, and records
[`demo.tape`](../demo.tape). The tape puts `target/release` first on `PATH` and
sets `GBAT_BINARY` to that same build, so all four commands use it. There is no
sample-output wrapper.

The recorder passes a temporary output path to VHS and checks that FFmpeg can
decode the entire GIF before moving it to `assets/demo.gif`.

Use VHS `0.12.1` or later: `0.12.0` can exit successfully without rendering,
which leaves the existing GIF untouched.

The checked-in capture used VHS `0.12.1`, ttyd `1.7.7`, and FFmpeg `9.0.2`.

## Check before updating the README

- Open the terminal animation and check its version, battery output, and
  readability. A successful recorder exit alone is not enough.
- Preserve the original Raycast animation byte-for-byte during terminal demo
  and release updates. Replace it only when explicitly requested.
- Check every relative link and image reference in both README files.
- Record the capture date, CLI version, observed output, and tool versions here.
  Both README captions link to this file.
