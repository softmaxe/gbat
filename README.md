<p align="center">
  <img src="assets/gbat-logo.png" alt="gbat logo" width="180">
</p>

<h1 align="center">gbat</h1>

<p align="center">
  <a href="README.md"><kbd>English</kbd></a>
  <a href="README.zh-CN.md"><kbd>简体中文</kbd></a>
</p>

`gbat` reads the battery level and charging state of a Logitech G Pro Wireless 2
on macOS, prints one line, and exits.

```text
Battery: 78%
Battery: 42% (charging)
```

Use it in Terminal, shell scripts, or Raycast. It does not require Logitech
G HUB, Python, or a background process.

## Requirements

- macOS 11 Big Sur or later
- A Logitech G Pro Wireless 2 connected through its LIGHTSPEED receiver or USB
- A mouse that is awake when `gbat` runs

## Install

The Homebrew package and GitHub release archive support Apple Silicon only,
with an `arm64` binary.

Install with Homebrew:

```sh
brew install softmaxe/tap/gbat
gbat --version
```

To upgrade:

```sh
brew upgrade gbat
```

To uninstall:

```sh
brew uninstall gbat
```

Release binaries have no Apple Developer ID signature or notarization. If macOS
blocks `gbat`, follow the steps in [Troubleshooting](#troubleshooting).

The [release workflow](.github/workflows/release.yml) publishes a SHA-256 checksum
and GitHub build provenance for each release archive. Find the archives on the
[releases page](https://github.com/softmaxe/gbat/releases). These checks do not
replace Apple code signing or notarization.

## Use

Run `gbat` with the mouse connected through its LIGHTSPEED receiver or USB:

```sh
gbat
```

On success, the command writes one battery status line to stdout and exits with
status `0`. On failure, it writes an error to stderr and exits with status `1`.
Run `gbat --version` to print the installed version without accessing the mouse.

An idle mouse may take longer to answer while its radio wakes. If the receiver
reports that the mouse is offline, turn it on or move it, then retry.

<p align="center">
  <img src="assets/demo.gif" alt="gbat version, battery output, and Raycast script demo" width="700">
</p>

This terminal demo shows real battery readings from `gbat 1.0.0`. It is a
historical recording, so its version may differ from the current release.
See [recording details](docs/recording.md) for its source and update instructions.

## Raycast

[`raycast/mouse-battery.sh`](raycast/mouse-battery.sh) is a Raycast Script Command.
Homebrew installs the binary only. Download the script into a local directory,
then make it executable with `chmod +x /path/to/mouse-battery.sh`. You can also
use this repository's `raycast` directory. Add the script's directory in Raycast
Settings, then run `Logitech Mouse Battery`.

<p align="center">
  <img src="assets/raycast-demo.webp" alt="gbat Raycast script command demo" width="700">
</p>

The script checks `PATH`, `/opt/homebrew/bin`, `/usr/local/bin`,
`$HOME/.local/bin`, and the repository's `target/release` and root directories,
in that order.

For a custom binary path, add this line to `mouse-battery.sh` before
`set -euo pipefail`:

```sh
export GBAT_BINARY="/path/to/gbat"
```

Setting this variable in Terminal only affects scripts launched from that shell.

Explicit paths take priority over the search above. The script checks
`GBAT_BINARY`, then the legacy variables `GPWBAT_BINARY` and
`GPW2_BATTERY_BINARY`, and uses the first non-empty value. That path must point
to an executable file.

## Build from source

Install Rust through `rustup`. The repository pins its Rust version in
[`rust-toolchain.toml`](rust-toolchain.toml), and `rustup` installs that toolchain
when needed. Then clone and build:

```sh
git clone https://github.com/softmaxe/gbat.git
cd gbat
cargo build --release --locked
./target/release/gbat
```

To keep the binary outside the build directory:

```sh
mkdir -p "$HOME/.local/bin"
cp target/release/gbat "$HOME/.local/bin/gbat"
```

Make sure `$HOME/.local/bin` is on `PATH` to run `gbat` from any directory.

## Troubleshooting

| Problem | What to do |
| --- | --- |
| `No responsive Logitech HID++ interface found` | Connect the receiver or USB cable, wake the mouse, and retry. |
| `The receiver reports no connected mouse` | The mouse is offline. Turn it on or move it, and retry. |
| `Could not read battery level` | The mouse may have gone offline during the read, or its battery response could not be used. Move it and retry. |
| `Could not initialize HID access` or an access error | Run `gbat` once from Terminal and approve any macOS permission prompt. `sudo` is not normally required. |
| `gbat binary not found` in Raycast | Install or build `gbat`, or set `GBAT_BINARY` in the script to its executable path. |
| `Battery: 100%` without `(charging)` | A full mouse may stop active charging. This is expected. |
| macOS blocks the binary | Open System Settings > Privacy & Security and choose Open Anyway for `gbat`. |

If Open Anyway does not work for a Homebrew installation, remove quarantine
from that formula only:

```sh
xattr -dr com.apple.quarantine "$(brew --prefix gbat)"
```

## How it works

`gbat` opens Logitech HID++ interfaces in shared mode. It checks receiver device
indices `1` through `6` and the direct USB index `0xFF`, trying `1` and `0xFF`
first. It uses the first interface and device index that reports a supported
battery feature. There is no device selector for setups with multiple mice.

It reads `UNIFIED_BATTERY` at `0x1004` and falls back to `BATTERY_STATUS` at
`0x1000` when the unified feature is unavailable, does not answer, or returns an
incomplete response. HID access errors and incomplete writes stop the read.
If neither feature yields a battery status, the command reports an error
instead of inventing a `0%` reading.

## License

[GNU Affero General Public License v3](LICENSE), `AGPL-3.0-only`.
