# 跨平台编译指南

本项目用 Rust + GPUI Kit 编写，一套代码可以发布到 Windows、macOS 和 Linux。
GPUI 的底层图形栈（Vulkan / Metal / D3D12）在不同平台差异很大，**几乎所有平台
都必须在对应操作系统上构建**。本文件说明每个目标平台的做法，以及哪些组合可以
在 Linux 上交叉编译。

---

## 1. 构建矩阵

| 目标平台 | 构建系统 | 产物 | 最低系统 |
| --- | --- | --- | --- |
| Windows 11 x64 | Windows + MSVC 工具链 | `.exe` 安装包 + 便携 `.zip` | Windows 10 1809+ |
| macOS 11+ Intel | macOS (x86_64) | `.dmg` + `.app` | macOS 11 |
| macOS 11+ Apple Silicon | macOS (arm64) | `.dmg` + `.app` | macOS 11 |
| Linux x86_64 | Linux | `.deb` / `.AppImage` / `.tar.gz` | glibc 2.31+ |
| Linux aarch64 | Linux (原生或交叉) | `.deb` / `.AppImage` / `.tar.gz` | glibc 2.31+ |

> Windows 的 Arm64 与 macOS 的 x86_64 / arm64 产物由各自主机分别构建，
> 不做交叉编译。理由：MSVC 工具链与 macOS 框架（Metal、AppKit、Notary）
> 都只能在各自的宿主系统上完整工作。

---

## 2. 各平台环境准备

### 2.1 Windows 11

```powershell
# 1. 安装 Rust（MSVC 工具链，不要用 GNU 版）
winget install Rustlang.Rustup
rustup default stable-x86_64-pc-windows-msvc
rustup target add x86_64-pc-windows-msvc

# 2. 安装 Visual Studio 2022 Build Tools（勾选“使用 C++ 的桌面开发”）
winget install Microsoft.VisualStudio.2022.BuildTools `
  --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# 3. 构建 + 打包
cargo build --release --locked
pwsh -File packaging\windows\build-windows.ps1
```

CMake 需要在 `PATH` 上（部分依赖用它找原生库）。

### 2.2 macOS 11+

```bash
# 1. Xcode 命令行工具
xcode-select --install

# 2. Rust 工具链（两个架构都要能构建）
rustup target add aarch64-apple-darwin x86_64-apple-darwin

# 3. 构建 + 打包
cargo build --release --locked
ARCH=aarch64 ./packaging/macos/build-macos.sh   # Apple silicon
ARCH=x86_64  ./packaging/macos/build-macos.sh   # Intel
```

发布给其他用户时需要签名与公证：

```bash
export SIGN_IDENTITY="Developer ID Application: Your Name (TEAMID)"
export NOTARY_PROFILE="notary-profile"     # xcrun notarytool store-credentials
./packaging/macos/build-macos.sh
```

### 2.3 Linux（含 CachyOS / Arch 系）

```bash
sudo pacman -S --needed base-devel cmake pkgconf \
    fontconfig wayland xorg-xwayland vulkan-icd-loader libxkbcommon \
    alsa-lib openssl zstd
```

Debian / Ubuntu：

```bash
sudo apt install -y build-essential clang cmake pkg-config \
    libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev \
    libssl-dev libzstd-dev libasound2-dev libvulkan1
```

打包：

```bash
cargo build --release --locked
./packaging/linux/build-deb.sh        # .deb + 便携 .tar.gz
./packaging/linux/build-appimage.sh   # .AppImage（需要 appimagetool）
```

---

## 3. 关于 Linux → macOS / Windows 的交叉编译

技术上不推荐，这里说明原因，避免团队里有人踩坑：

* **GPUI Kit 的 Linux 平台层只编译 `x86_64-unknown-linux-gnu`。**
  `gpui-pre-linux` 没有 Windows/macOS 的条件编译分支，无法在 Linux 上
  产出对应平台的二进制。
* **macOS**：需要 Metal、AppKit、CoreFoundation 与 Apple 的代码签名/公证
  工具链（`codesign`、`xcrun notarytool`），只在 macOS 上可用。
* **Windows**：需要 MSVC 的链接器与 Windows SDK；GPUI 的 Windows 平台层
  使用 Direct3D 与 Win32 窗口，Wine/QEMU 交叉编译不可靠。
* **AppImage**：只能构建 Linux 二进制，但可以用 `linuxdeploy` 在容器里为
  其他架构（x86_64 / aarch64）准备运行时依赖。

如果要覆盖多架构，推荐用 **GitHub Actions 的 `runs-on: matrix`**（见
`.github/workflows/release.yml`），每个平台用官方 runner 构建。

---

## 4. 代码签名与公证速查

| 平台 | 未签名 | 正式发布 |
| --- | --- | --- |
| Windows | 可直接运行（SmartScreen 会告警） | `signtool sign /fd SHA256 /tr http://timestamp.digicert.com ...`；先签 exe 再封包 |
| macOS | 仅本机可运行 | Developer ID 签名 + `notarytool` 公证 + `stapler` 装订 |
| Linux | AppImage / deb 无需签名 | 可选 GPG 签名仓库元数据；AppImage 可用 `zsync` |

签名证书和公证凭据**不要**提交进仓库，统一走 CI Secrets。

---

## 5. 版本号只有一个来源

`Cargo.toml` 的 `version` 是唯一版本号来源：

* Linux 打包脚本读取它写入 `control` / AppImage 元数据；
* `Info.plist` 在构建时用 `PlistBuddy` 同步；
* Inno Setup 用 `#define AppVersion "0.1.0"`，**发布前记得同步**，
  或改为从文件读取。

发版前检查清单：

1. `cargo build --release --locked` 通过；
2. `cargo clippy --all-targets -- -D warnings` 通过；
3. 更新 `packaging/windows/course-plan.iss` 的 `AppVersion`；
4. 更新 `packaging/macos/course-plan.metainfo.xml`（若存在）与 AppImage 元数据的 release 条目；
5. 在干净机器上安装并运行产物，验证升级与卸载。
