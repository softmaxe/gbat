<p align="center">
  <img src="assets/gbat-logo.png" alt="gbat 标志" width="180">
</p>

<h1 align="center">gbat</h1>

<p align="center">
  <a href="README.md"><kbd>English</kbd></a>
  <a href="README.zh-CN.md"><kbd>简体中文</kbd></a>
</p>

`gbat` 在 macOS 上读取 Logitech G Pro Wireless 2 鼠标的电量和充电状态，输出一行结果后退出。

```text
Battery: 78%
Battery: 42% (charging)
```

可在 Terminal、Shell 脚本或 Raycast 中使用，不需要 Logitech G HUB、Python 或后台进程。

## 系统要求

- macOS 11 Big Sur 或更高版本
- 通过 LIGHTSPEED 接收器或 USB 连接的 Logitech G Pro Wireless 2
- 运行 `gbat` 时鼠标处于唤醒状态

## 安装

Homebrew 软件包和 GitHub Release 归档仅提供 Apple Silicon 的 `arm64` 可执行文件。

使用 Homebrew 安装：

```sh
brew install softmaxe/tap/gbat
gbat --version
```

升级：

```sh
brew upgrade gbat
```

卸载：

```sh
brew uninstall gbat
```

Release 可执行文件没有 Apple Developer ID 签名，也未经过 Apple 公证。如果 macOS 阻止运行，请按[故障排查](#故障排查)中的步骤处理。

[发布工作流](.github/workflows/release.yml)会为每个 Release 归档生成 SHA-256 校验和及 GitHub 构建来源证明。归档文件见 [Release 页面](https://github.com/softmaxe/gbat/releases)。这些校验不能替代 Apple 代码签名或公证。

## 使用

通过 LIGHTSPEED 接收器或 USB 连接鼠标，然后运行：

```sh
gbat
```

成功时，命令向 stdout 输出一行电池状态，退出码为 `0`。失败时，命令向 stderr 输出错误，退出码为 `1`。运行 `gbat --version` 可查看已安装的版本，不会访问鼠标。

鼠标闲置后，读取可能需要等待无线模块唤醒。如果接收器报告鼠标离线，打开鼠标电源或移动鼠标后重试。

<p align="center">
  <img src="assets/demo.gif" alt="gbat 版本、电量输出和 Raycast 脚本演示" width="700">
</p>

这段终端演示使用 `gbat 1.0.0` 读取真实鼠标电量。录屏保留了当时的版本，可能与当前 Release 不同。源码版本和更新方法见[录屏说明](docs/recording.md)。

## Raycast

[`raycast/mouse-battery.sh`](raycast/mouse-battery.sh) 是 Raycast Script Command。Homebrew 只安装 `gbat` 可执行文件。将脚本下载到本地目录后，运行 `chmod +x /path/to/mouse-battery.sh` 添加执行权限。也可以使用本仓库的 `raycast` 目录。在 Raycast Settings 中添加脚本所在目录，然后运行 `Logitech Mouse Battery`。

<p align="center">
  <img src="assets/raycast-demo.webp" alt="gbat Raycast Script Command 演示" width="700">
</p>

脚本依次检查 `PATH`、`/opt/homebrew/bin`、`/usr/local/bin`、`$HOME/.local/bin`，以及仓库的 `target/release` 目录和根目录。

如果可执行文件位于其他位置，在 `mouse-battery.sh` 的 `set -euo pipefail` 之前加入：

```sh
export GBAT_BINARY="/path/to/gbat"
```

在 Terminal 中设置这个变量，只会影响从该 Shell 启动的脚本。

指定路径的优先级高于上述搜索。脚本依次检查 `GBAT_BINARY`、旧变量 `GPWBAT_BINARY` 和 `GPW2_BATTERY_BINARY`，使用第一个非空值。该路径必须指向可执行文件。

## 从源码构建

通过 `rustup` 安装 Rust。仓库在 [`rust-toolchain.toml`](rust-toolchain.toml) 中指定 Rust 版本，`rustup` 会在需要时安装对应工具链。然后克隆仓库并构建：

```sh
git clone https://github.com/softmaxe/gbat.git
cd gbat
cargo build --release --locked
./target/release/gbat
```

如需将可执行文件保存在构建目录之外：

```sh
mkdir -p "$HOME/.local/bin"
cp target/release/gbat "$HOME/.local/bin/gbat"
```

确认 `$HOME/.local/bin` 已加入 `PATH`，即可在任意目录运行 `gbat`。

## 故障排查

| 问题 | 处理方法 |
| --- | --- |
| `No responsive Logitech HID++ interface found` | 连接接收器或 USB 线，唤醒鼠标后重试。 |
| `The receiver reports no connected mouse` | 鼠标已离线。打开鼠标电源或移动鼠标后重试。 |
| `Could not read battery level` | 鼠标可能在读取时离线，或返回了无法使用的电池响应。移动鼠标后重试。 |
| `Could not initialize HID access` 或访问错误 | 从 Terminal 运行一次 `gbat`，并允许 macOS 弹出的权限请求。通常不需要 `sudo`。 |
| Raycast 提示 `gbat binary not found` | 安装或构建 `gbat`，或在脚本中将 `GBAT_BINARY` 设为它的可执行文件路径。 |
| `Battery: 100%` 但没有 `(charging)` | 鼠标充满后可能停止主动充电，这是正常现象。 |
| macOS 阻止运行 | 打开 System Settings > Privacy & Security，为 `gbat` 选择 Open Anyway。 |

如果通过 Homebrew 安装后仍无法使用 Open Anyway 运行，只清除该 Formula 的隔离属性：

```sh
xattr -dr com.apple.quarantine "$(brew --prefix gbat)"
```

## 工作原理

`gbat` 以共享模式打开 Logitech HID++ 接口。它会检查接收器的设备索引 `1` 到 `6` 和 USB 直连索引 `0xFF`，先尝试 `1` 和 `0xFF`，再使用首个报告支持电池功能的接口及设备索引。连接多只鼠标时，无法指定要读取哪一只。

它优先读取 `0x1004` 的 `UNIFIED_BATTERY`。该功能不可用、未响应或返回不完整响应时，会回退到 `0x1000` 的 `BATTERY_STATUS`。HID 访问错误和写入不完整会直接终止读取。如果两个功能都未返回电池状态，命令会报错，不会将读取失败显示为 `0%`。

## 许可证

[GNU Affero General Public License v3](LICENSE)，`AGPL-3.0-only`。
