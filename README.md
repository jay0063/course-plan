# 小学课程管理计划 · CoursePlan

一个用 Rust + [GPUI Kit](https://gpui-kit.com) 写的跨平台桌面应用，帮家长把孩子的
**学校课程**和**补习课程**管起来：录入课程 → 生成每周计划表 → 到点提醒 → 打卡记录。

界面为中文，数据全部保存在本地，不联网。

---

## 功能

| 模块 | 说明 |
| --- | --- |
| **今日概览** | 今日课程时间轴（含“进行中”高亮）、今日打卡、待办提醒、本周课时分布图、快捷入口 |
| **课程管理** | 学校课程 / 补习课程分类，8 种颜色标识，记录科目、老师、每周课时 |
| **计划表** | 每周 7 天网格课表，点击任意格子即可排课，右侧显示校内/补习课时汇总 |
| **日程安排** | 按日期管理一次性日程，同日显示当天课程，可标记完成 |
| **提醒事项** | 支持「仅一次 / 每天 / 每周」重复，到期弹出系统通知，区分“即将到期”和“已超时” |
| **打卡任务** | 三栏看板（待打卡 / 今日已完成 / 全部任务），记录连续天数、累计次数与完成率 |
| **备忘笔记** | 彩色卡片，支持置顶，按更新时间排序 |
| **设置** | 学生资料、学期起止、提醒开关、浅色/深色主题、数据统计与清空 |

首次启动会写入一份示例数据（7 门课程、16 条课表安排、4 个打卡任务、2 条备忘），
打开即可看到完整效果。在「设置 → 清空全部数据」中可以还原为空白状态。

---

## 技术栈

* **Rust 2024** + [GPUI Kit 0.7](https://gpui-kit.com)（GPUI 的组件库封装）
* 图标为 Lucide，内嵌在二进制中
* 数据持久化：单个 JSON 文件，原子写入（先写 `.tmp` 再 `rename`）
* 无网络请求，无遥测

---

## 快速开始

### 依赖

```bash
# Rust 1.92 或更高（GPUI Kit 的 Linux 依赖要求）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Debian / Ubuntu
sudo apt install -y build-essential clang cmake pkg-config \
    libfontconfig-dev libwayland-dev libxkbcommon-x11-dev libx11-xcb-dev \
    libssl-dev libzstd-dev libasound2-dev libvulkan1

# CachyOS / Arch
sudo pacman -S --needed base-devel cmake pkgconf fontconfig wayland \
    vulkan-icd-loader libxkbcommon alsa-lib
```

### 开发运行

```bash
cargo run
```

Linux 上必须在图形会话（Wayland 或 X11）里运行，并且需要可用的 Vulkan 驱动；
只有 Vulkan loader 而没有驱动是渲染不出来的。

### 质量检查

```bash
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo build --release --locked
```

---

## 打包发布

完整的多平台编译说明见 [`packaging/CROSS-COMPILE.md`](packaging/CROSS-COMPILE.md)。
一句话版本：**每个平台都在各自的系统上构建**（GPUI 的图形后端不同）。

### Linux（CachyOS / Debian / Ubuntu / Arch）

```bash
cargo build --release --locked

./packaging/linux/build-deb.sh         # dist/course-plan_<version>_amd64.deb + 便携 tar.gz
./packaging/linux/build-appimage.sh    # dist/course-plan-<version>-x86_64.AppImage
```

安装测试：

```bash
sudo apt install ./dist/course-plan_0.1.0_amd64.deb
```

`build-deb.sh` 会用 `dpkg-shlibdeps` 从**实际构建出的二进制**推导运行时依赖，
再补上 Vulkan 与图形栈。`.deb` 会注册桌面菜单项和图标，`sudo apt remove course-plan`
可以干净卸载。

### Windows 11

```powershell
cargo build --release --locked
pwsh -File packaging\windows\build-windows.ps1
```

需要 **MSVC 工具链**（不是 GNU）、Visual Studio 2022 Build Tools 的
“使用 C++ 的桌面开发”工作负载，以及 Inno Setup 6。产物：

* `dist\CoursePlan-<version>-windows-x64.zip` — 便携版，解压即用
* `dist\CoursePlan-Setup-<version>.exe` — 安装包，带开始菜单/桌面快捷方式与卸载项

### macOS 11+（Intel 与 Apple Silicon）

```bash
ARCH=aarch64 ./packaging/macos/build-macos.sh   # Apple silicon
ARCH=x86_64  ./packaging/macos/build-macos.sh   # Intel
```

产物：`.app` 包、`.dmg`、以及供公证用的 `.app.tar.gz`。
设置 `SIGN_IDENTITY` 与 `NOTARY_PROFILE` 环境变量即可完成签名与公证。

### CI

`.github/workflows/release.yml` 用 GitHub Actions 的 matrix 同时构建
Linux / Windows / macOS(arm64 + x86_64)，推 tag 即发布。

---

## 项目结构

```
src/
├── main.rs           # 入口：初始化、窗口、身份
├── app.rs            # 应用外壳：侧边栏、顶栏、导航、提醒循环
├── model.rs          # 数据模型：课程/课表/日程/提醒/打卡/备忘
├── store.rs          # 持久化：JSON 读写 + 首次运行的示例数据
├── prelude.rs        # 统一的 import 集合
├── ui/
│   ├── widgets.rs    # 卡片、统计块、标签、配色板 Skin 等
│   ├── select.rs     # 自定义下拉选择器
│   └── form.rs       # 表单行
└── views/
    ├── dashboard.rs  # 今日概览
    ├── courses.rs    # 课程管理
    ├── schedule.rs   # 计划表（周课表网格）
    ├── events.rs     # 日程安排
    ├── reminders.rs  # 提醒事项
    ├── checkins.rs   # 打卡任务（看板）
    ├── memos.rs      # 备忘笔记
    └── settings.rs   # 设置

packaging/
├── icons/            # SVG 源图 + 各尺寸 PNG + .ico / .icns
├── linux/            # desktop entry、AppRun、AppImage 与 deb 打包脚本
├── windows/          # Inno Setup 脚本 + PowerShell 打包脚本
├── macos/            # Info.plist + .app/.dmg 打包脚本
└── CROSS-COMPILE.md  # 跨平台编译指南
```

### 架构约定

* 每个页面是一个普通函数 `render(&mut App, &mut Context<App>) -> impl IntoElement`；
  需要交互的局部状态（输入框、下拉框）各自是一个 `Entity<...>` 视图。
* 编辑器通过 `Rc<dyn Fn(Draft, &mut Window, &mut gpui::App)>` 回调把结果交回 `App`，
  `App` 负责校验、写库和提示。
* 所有写操作都走 `App::commit`，它负责保存 JSON 并弹通知，失败不会丢数据。

---

## 数据存放位置

| 平台 | 路径 |
| --- | --- |
| Linux | `~/.local/share/courseplan/course-plan/data.json` |
| macOS | `~/Library/Application Support/com.courseplan.course-plan/data.json` |
| Windows | `%APPDATA%\courseplan\course-plan\data.json` |

卸载应用不会删除数据文件，需要时可在设置页手动清空。

---

## 已知限制

* 图标只在带文字的控件里绘制，纯图标按钮在 GPUI Kit 0.7.1 上不显示，
  因此行内操作统一使用「编辑 / 删除」文字按钮（同时对读屏软件更友好）。
* 提醒循环每 30 秒轮询一次，精度到分钟级；系统通知依赖平台的通知服务，
  Linux 上需要 `libnotify`/桌面通知守护进程。
* 课程表按「每周重复」建模，暂不支持单周调课与节假日例外。

## 许可

Apache-2.0
