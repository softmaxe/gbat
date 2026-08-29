<p align="center">
  <img src="assets/gpw2-battery-logo.png" alt="gpw2-battery logo" width="180">
</p>

<h1 align="center">gpw2-battery</h1>

<p align="center">
  <a href="README.md"><kbd>English</kbd></a>
  <a href="README.zh-CN.md"><kbd>简体中文</kbd></a>
</p>

gpw2-battery is a small macOS command-line tool for reading the current battery level and active charging state of a Logitech G Pro Wireless 2 mouse.

It talks to the mouse through the Logitech HID++ 2.0 vendor interface. It does not need Logitech G HUB, Python, or a background process. The same binary works from Terminal and from Raycast.

The macOS hidapi backend uses shared device access, so separate Terminal and Raycast invocations can open the HID interface without an exclusive-open conflict.

## Supported connections

- Logitech LIGHTSPEED receiver
- Direct USB connection

The mouse must be awake and connected to the Mac when the command runs.

## Build

Rust is needed to build the project. A prebuilt binary can be used without installing Rust.

~~~
cargo build --release
~~~

The binary is written to target/release/gpw2-battery.

## Install with Homebrew

Install the architecture-specific release through the existing tap:

~~~
brew install softmaxe/tap/gpw2-battery
gpw2-battery --version
~~~

Homebrew selects the arm64 package on Apple Silicon and the x86_64 package on Intel. Upgrade or remove it with:

~~~
brew upgrade gpw2-battery
brew uninstall gpw2-battery
~~~

The formula puts `gpw2-battery` on Homebrew's `bin` directory, so it can be called from Terminal and Raycast.

## Use from Terminal

Run the binary directly from the project:

~~~
./target/release/gpw2-battery
~~~

Successful output is one line:

~~~
Battery: 78%
Battery: 42% (charging)
~~~

Errors go to stderr and return a non-zero exit status. This keeps stdout suitable for shell scripts and Raycast compact output.

To use the command from any directory, copy the release binary to a directory on your PATH, for example:

~~~
mkdir -p "$HOME/.local/bin"
cp target/release/gpw2-battery "$HOME/.local/bin/gpw2-battery"
~~~

## Use from Raycast

The repository includes raycast/mouse-battery.sh as a Raycast Script Command. Add the repository's raycast directory under Raycast Settings, then run Logitech Mouse Battery.

The wrapper looks for the binary in this order:

1. `GPW2_BATTERY_BINARY`
2. `command -v gpw2-battery`
3. `/opt/homebrew/bin/gpw2-battery`
4. `/usr/local/bin/gpw2-battery`
5. `$HOME/.local/bin/gpw2-battery`
6. `target/release/gpw2-battery` in this project
7. `gpw2-battery` in the project directory

Set GPW2_BATTERY_BINARY when the binary lives somewhere else:

~~~
export GPW2_BATTERY_BINARY="/path/to/gpw2-battery"
~~~

The wrapper executes the binary directly. It never runs cargo run.

## How it works

The command enumerates Logitech HID interfaces with vendor ID 0x046D and usage page 0xFF00. It probes device indexes 1, 2, 3, 4, 5, 6, and 0xFF, which covers receiver and direct USB paths used by the GPW2.

For each responsive interface it asks the HID++ ROOT feature (0x0000) for the feature index of UNIFIED_BATTERY (0x1004). That feature is queried first. If it is unavailable or does not return a usable response, the command falls back to BATTERY_STATUS (0x1000). The battery level is read from response byte resp[4]. The active charging bit is read from resp[7] & 1 for UNIFIED_BATTERY and resp[6] & 1 for BATTERY_STATUS.

The command accepts short HID++ reports only when they contain the bytes needed for the requested value. Error reports and missing responses are treated as failures, never as 0%.

## Troubleshooting

No responsive Logitech HID++ interface found means that no matching interface answered a battery feature probe. Check that the receiver is connected or the mouse is attached by USB, then wake the mouse and try again.

Could not initialize HID access or an access error can indicate a macOS permission issue. Run the command once from Terminal and approve any prompt macOS shows. The command does not require sudo in normal setups.

If the mouse has gone to sleep, move it or click a button before retrying. A full battery may report Battery: 100% without (charging) because the mouse is no longer actively charging.

Release binaries are unsigned and are not notarized by Apple. If macOS blocks the binary, first open System Settings, then Privacy & Security, and choose Open Anyway for gpw2-battery. If that does not work, remove quarantine only from the Homebrew-installed formula:

~~~
xattr -dr com.apple.quarantine "$(brew --prefix gpw2-battery)"
~~~

GitHub build provenance attestations identify the workflow and source repository that produced a release asset. They do not replace an Apple Developer ID signature or notarization.
