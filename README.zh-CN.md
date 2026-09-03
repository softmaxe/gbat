<p align="center">
  <img src="assets/gbat-logo.png" alt="gbat logo" width="180">
</p>

<h1 align="center">gbat</h1>

<p align="center">
  <a href="README.md"><kbd>English</kbd></a>
  <a href="README.zh-CN.md"><kbd>简体中文</kbd></a>
</p>

在 macOS 上读取 Logitech G Pro Wireless 2 鼠标的电量和充电状态。

```text
Battery: 78%
Battery: 42% (charging)
```

- 支持 LIGHTSPEED 接收器和 USB 直连
- 可在 Terminal 和 Raycast 中使用
- 不需要 Logitech G HUB、Python 或后台进程
- Terminal 和 Raycast 可共享 HID 设备访问

## 系统要求

- macOS 11（Big Sur）或更高版本
- 通过 LIGHTSPEED 接收器或 USB 连接的 Logitech G Pro Wireless 2
- 运行 `gbat` 时鼠标处于唤醒状态

## 安装

Homebrew 软件包和 GitHub Release 归档仅支持 Apple Silicon（`arm64`）。Release 可执行文件没有签名，也没有经过 notarization，因此 macOS 可能要求你在 System Settings > Privacy & Security 中批准该可执行文件。

使用 Homebrew 安装 Apple Silicon 可执行文件：

```sh
brew install softmaxe/tap/gbat
gbat --version
```

升级或卸载：

```sh
brew upgrade gbat
brew uninstall gbat
```

## 使用

通过 LIGHTSPEED 接收器或 USB 连接鼠标，然后运行：

```sh
gbat
```

命令会向 stdout 输出一行电池状态。错误写入 stderr，并返回非零退出状态，因此可以直接用于脚本。

<p align="center">
  <img src="assets/demo.gif" alt="gbat CLI 演示" width="700">
</p>

## Raycast

[`raycast/mouse-battery.sh`](raycast/mouse-battery.sh) 是 Raycast Script Command。在 Raycast Settings 中添加本仓库的 `raycast` 目录，然后运行 `Logitech Mouse Battery`。

<p align="center">
  <img src="assets/raycast-demo.webp" alt="gbat Raycast 脚本命令演示" width="700">
</p>

脚本会在 `PATH`、Homebrew 默认目录、`$HOME/.local/bin` 和当前项目中查找 `gbat`。如果可执行文件位于其他位置，请指定路径：

```sh
export GBAT_BINARY="/path/to/gbat"
```

旧变量 `GPWBAT_BINARY` 和 `GPW2_BATTERY_BINARY` 仍可使用。

## 从源码构建

构建需要 Rust：

```sh
cargo build --release
./target/release/gbat
```

如需在任意目录运行 `gbat`，将可执行文件复制到 `PATH` 中的目录：

```sh
mkdir -p "$HOME/.local/bin"
cp target/release/gbat "$HOME/.local/bin/gbat"
```

## 故障排查

| 问题 | 处理方法 |
| --- | --- |
| `No responsive Logitech HID++ interface found` | 连接接收器或 USB 线，唤醒鼠标后重试。 |
| `Could not initialize HID access` 或 access error | 从 Terminal 运行一次 `gbat`，并允许 macOS 弹出的权限请求。通常不需要 `sudo`。 |
| `Battery: 100%` 但没有 `(charging)` | 鼠标充满后可能停止主动充电，这是正常现象。 |
| macOS 阻止运行 | 打开 System Settings > Privacy & Security，为 `gbat` 选择 Open Anyway。 |

如果 Open Anyway 对 Homebrew 安装无效，只清除该 Formula 的 quarantine：

```sh
xattr -dr com.apple.quarantine "$(brew --prefix gbat)"
```

## 工作原理

`gbat` 通过 Logitech HID++ 2.0 厂商接口通信。它会探测 Logitech HID++ 接口，检查设备索引 `1` 到 `6` 以及 `0xFF`，优先读取 `UNIFIED_BATTERY` (`0x1004`)，不可用时回退到 `BATTERY_STATUS` (`0x1000`)。无效、不完整或缺失的响应会返回错误，不会显示为 `0%`。

每个 Release 都会为归档文件单独发布 SHA-256 checksum 和 GitHub build provenance。这些内容不能替代 Apple 代码签名或 notarization。详见 [releases](https://github.com/softmaxe/gbat/releases) 和 [release workflow](.github/workflows/release.yml)。

## 许可证

[GNU Affero General Public License v3](LICENSE)，`AGPL-3.0-only`。
