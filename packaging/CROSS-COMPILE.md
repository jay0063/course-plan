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

### 2.1 Windows 10 1809+ / Windows 11 (x64)

#### 环境准备

```powershell
# 1. Rust（MSVC 工具链，不要用 GNU 版）
winget install Rustlang.Rustup
rustup default stable-x86_64-pc-windows-msvc
rustup target add x86_64-pc-windows-msvc

# 2. Visual Studio 2022 Build Tools（勾选“使用 C++ 的桌面开发”/ VCTools 工作负载）
winget install Microsoft.VisualStudio.2022.BuildTools `
  --override "--wait --passive --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended"

# 3. Inno Setup 6（生成安装包）
winget install JRSoftware.InnoSetup
```

* **CMake** 必须能在 `PATH` 上找到（部分依赖用它定位原生库）。
* **显卡 / 驱动**：GPUI 在 Windows 上走 Direct3D 12（wgpu），需要支持 D3D12 的
  显卡与较新的驱动。**不依赖 WebView2**，也不依赖任何系统 WebView。
* 仓库是 Cargo workspace，**根包就是客户端**（`src/`）。在仓库根执行
  `cargo build --release --locked` 只构建客户端；服务端是 `crates/server`，不参与桌面打包。

#### 构建与打包

```powershell
# 默认：x86_64-pc-windows-msvc + 动态 CRT + 便携 zip + 安装包
pwsh -File packaging\windows\build-windows.ps1

# 只要便携 zip，不生成安装包（也不要求装 Inno Setup）
pwsh -File packaging\windows\build-windows.ps1 -SkipInstaller

# 静态 CRT：便携包不再依赖 VC++ 运行库（见下）
pwsh -File packaging\windows\build-windows.ps1 -StaticCrt

# 指定目标 triple（默认 x86_64-pc-windows-msvc）
pwsh -File packaging\windows\build-windows.ps1 -Target aarch64-pc-windows-msvc
```

脚本自身会执行 `cargo build --release --locked --target <triple>`。因为带了
`--target`，二进制落在 `target\<triple>\release\course-plan.exe`；脚本会自动识别
该路径，并以 `/DExePath=...` 的完整路径传给 Inno Setup（`.iss` 里不再硬编码
`target\release\`）。版本号从 `Cargo.toml` 读取，以 `/DAppVersion=...` 传入，因此
不会与 `Cargo.toml` 漂移（`.iss` 里的默认值只用于直接运行 ISCC 的场景）。

产物落在**仓库根**的 `dist\`：

| 产物 | 说明 |
| --- | --- |
| `dist\CoursePlan-<version>-windows-x64.zip` | 便携版，解压即用 |
| `dist\CoursePlan-Setup-<version>.exe` | Inno Setup 安装包（开始菜单 / 桌面快捷方式 / 卸载项） |

手动运行 ISCC（不经脚本）时，请显式传入版本与 exe 路径：

```powershell
ISCC.exe /DAppVersion=0.1.0 `
         /DExePath=target\x86_64-pc-windows-msvc\release\course-plan.exe `
         packaging\windows\course-plan.iss
```

> `.iss` 用 `SourceDir=..\..` 把相对路径锚定在仓库根（脚本位于
> `packaging\windows\`），所以 `OutputDir=dist`、`SetupIconFile=packaging\icons\...`、
> `README.md` 都相对仓库根解析，而不是相对脚本目录。

#### VC++ 运行库

默认（**动态 CRT**）下，目标机器必须安装
**「Microsoft Visual C++ 2015-2022 Redistributable (x64)」**，否则程序启动即报缺少
`VCRUNTIME140.dll` / `MSVCP140.dll`。两种做法二选一：

* **随包安装**：在安装包里附带 `vc_redist.x64.exe`，并用 Inno 的 `[Run]` 段静默安装；
  便携版则在说明里提示用户自行安装。
* **静态链接**：用 `-StaticCrt` 构建（在 `cargo build` 前设置
  `RUSTFLAGS="-C target-feature=+crt-static"`，脚本已封装）。CRT 被链进 exe，
  便携包在干净的 Windows 上可直接运行，代价是体积略大。

#### 代码签名（可选，但强烈推荐）

未签名的 exe 会触发 SmartScreen「未知发布者」警告。发布前建议先用 `signtool` 签
exe（**在 ISCC 之前**），再签安装包：

```powershell
signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 `
  /f cert.pfx /p "<password>" `
  target\x86_64-pc-windows-msvc\release\course-plan.exe
```

再在 `course-plan.iss` 的 `[Setup]` 里配置 Inno 的签名指令，使安装包与卸载器也被签名：

```ini
SignTool=signtool sign /fd sha256 /tr http://timestamp.digicert.com /td sha256 /f cert.pfx
SignedUninstaller=yes
```

证书（`.pfx`）与密码**不要**提交进仓库；CI 里通过 Secrets 注入（例如把证书
base64 存进 secret，构建时解码到临时文件）。未签名构建仍可运行，但会触发 SmartScreen。

#### 同步与 TLS 的注意事项

客户端用 `reqwest` 且启用 `rustls-tls`（内置 Mozilla 根证书 + 纯 Rust TLS，
不依赖系统 OpenSSL）。这意味着 **私有 / 自签名 CA 一律无法通过校验**——例如局域网里
Caddy 用 `internal` CA 签发的证书会握手失败。两条出路：

* 用公有 CA：Caddy 绑定真实域名自动申请 Let's Encrypt 证书（推荐）；
* 或给 `Cargo.toml` 的 `reqwest` 增加 `rustls-tls-native-roots` feature，让客户端改用
  Windows 证书存储里的根证书。**这需要在 `Cargo.toml` 改依赖，本文档仅作说明。**

#### Windows 侧冒烟清单

在干净的 Windows 10/11 x64 机器上：

1. 用便携包解压启动（或安装包安装后启动），**8 个页面**逐个切换不崩溃；
2. 「课程管理」新建一门课程并保存；
3. 确认数据文件出现在
   `%APPDATA%\courseplan\CoursePlan\data\course-plan\data.json`
   （**Roaming** AppData，而非 LocalAppData）；
4. 设置页切换浅色 / 深色主题正常；
5. （可选）用 `docker compose` 拉起服务端，在「设置 → 同步」里登录并确认多设备收敛；
6. **卸载应用 → 数据文件仍在**（安装包不删除用户数据）；
7. 如需彻底清除，手动删除整个 `%APPDATA%\courseplan` 目录。

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
* Windows：`build-windows.ps1` 读取它并以 `/DAppVersion` 传给 Inno Setup，
  **无需再手工同步**（`.iss` 里的 `#ifndef` 默认值只用于直接运行 ISCC 的场景）。

发版前检查清单：

1. `cargo build --release --locked` 通过；
2. `cargo clippy --all-targets -- -D warnings` 通过；
3. 确认 `Cargo.toml` 的 `version` 已 bump（各平台打包脚本都从这里读取）；
4. 更新 `packaging/macos/course-plan.metainfo.xml`（若存在）与 AppImage 元数据的 release 条目；
5. 在干净机器上安装并运行产物，验证升级、卸载**保留用户数据**，以及重装不丢数据。
