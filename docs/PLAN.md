# CoursePlan 重构 + 同步后端 — 实施计划

> 状态: 待 Oracle 评审 (Gate 0) · 日期: 2026-10-17 · 基线 commit: `7700204`

## 1. 目标

1. **全面分析**现有 GPUI 桌面应用（已完成侦察，见 §3）。
2. **产品重构**：整体设计系统重做（活力儿童向：高饱和、圆润、大触控目标、细腻动效），逐页重构 8 个视图的布局与交互。
3. **服务端数据同步**：Rust (axum) + SQLite 单容器后端，家庭账号（用户名+密码），实体级 LWW 同步 + 墓碑删除。
4. **容器化部署**：多阶段 Dockerfile + docker compose（含可选 Caddy TLS 叠加），文档化。

### 冻结决策（用户确认）

| 决策 | 结论 |
| --- | --- |
| 账号模型 | 家庭账号：用户名 + 密码（argon2id + JWT） |
| 后端存储 | SQLite 单容器 + 数据卷 |
| 视觉方向 | 活力儿童向 |

### 非目标（v1）

- 移动端 App、Web 前端、实时推送（WS/SSE）——轮询 + 手动同步即可
- 账号间共享/多家庭协作、端到端加密、限流/风控
- 日历单选周调课（保持每周重复模型）

### 显式假设（如有异议请尽早提出）

- 保留现有全部功能与中文 UI；重构的是结构、视觉、交互，不是功能裁剪
- 首次运行不再自动写入示例数据 → 改为显式「载入示例数据」动作（否则新设备登录会把示例数据推上服务端）
- 课程表「每周重复」模型保持不变
- 客户端仍为桌面端 GPUI 应用（Linux/Win/macOS 打包脚本保留）

## 2. 关键侦察结论（Phase 0 输入）

### 2.1 代码库现状

- 单 crate，17 个源文件 ~6.6k LOC；无 git 历史（已 `git init` 并提交基线）
- `src/model.rs`: `Database` = 一个大 JSON 文档（profile/courses/sessions/events/reminders/checkins/memos），实体无时间戳、无删除墓碑
- `src/store.rs`: 单 JSON 原子写；**损坏数据会被示例数据静默覆盖**（数据丢失风险，须修）
- `src/app.rs`: 单 `App` 实体持有 `db` + 全部 UI 状态；视图直接 `&mut App` 改数据后 `commit()` 落盘 → 重构点：所有写路径收口到带时间戳的 mutation API
- 已知交互缺陷 16 项（explorer 清单）：memos 空态丢失 CTA、overlay 六处重复且 memos 未居中、打卡归档无确认、主题不持久、无任何动效、日期时间全是裸文本输入、无键盘焦点、无间距/字号 token 等

### 2.2 gpui-kit 0.7.1 能力（librarian 在 vendor 源码中核实）

- **动效可用**：`gpui_kit::*` 重导出 `with_animation/Animation/ease_*`；`gpui_base{motion}` 提供 `transition()/spring()/Presence/Easing/Keyframes/Sequence/Stagger`；Dialog/Sheet/Notification/Skeleton 自带入场动画；`Theme.motion` 是 MotionTokens
- **组件远多于现用**：`Dialog/AlertDialog/Sheet/Popover/Tooltip/DatePicker/Calendar/TimeField/TabBar(segmented/pill/underline)/Skeleton/Progress/ProgressCircle/Switch/Badge/Tag/DataTable/chart(bar/pie/line)/color_picker` 等全部存在 → 裸文本日期输入应替换为 DatePicker/TimeField
- **图标不渲染的真正原因**：app 注册的 `gpui_kit_assets::Assets` 只内嵌 106 个图标，应用用到的 `CalendarDays/Clock/Flame/GraduationCap/ListTodo/NotebookPen/Target/Trash` 8 个不在其中，`Assets::load` 返回 Err → SVG 整块绘制失败（README 里「图标按钮不显示」的结论是误判）。修复：`icon_assets!` 宏构建自选图标集（或 AllAssets），并加「每个用到的图标必可加载」测试
- **HTTP 能力**：`Application::with_http_client` + gpui-pre 自带 `http_client` trait；reqwest 0.12.15 已在 lockfile。客户端同步引擎采用 **专用工作线程 + 自带 tokio runtime + smol channel 回传**（不依赖 UI 线程 executor 细节），reqwest 选用 `rustls-tls`（去掉 openssl 系统依赖）
- **无头验证可行**：`HeadlessAppContext` + `WgpuHeadlessRenderer`（VULKAN|GL），本机有 lavapipe (`lvp_icd.json`)；`capture_screenshot() -> RgbaImage`、`TestWindowExt`（click/input/hover/wait_for）。可对所有页面出截图 → 设计评审闭环
- **版本**：crates.io 最新 gpui-kit 0.7.3（0.7.1→0.7.3 API 几乎不变，新增 DialogEntrance、diff 等）→ 顺带升级

## 3. 目标仓库结构

```
course-plan/
├── Cargo.toml              # workspace: [".", "crates/sync-proto", "crates/server"]
├── src/
│   ├── main.rs             # 薄二进制 → course_plan::run()
│   ├── lib.rs              # pub mod 导出（供集成测试/视觉测试）
│   ├── model.rs            # 实体 + updated_at/deleted_at  + Mutation API
│   ├── store.rs            # 落盘 / 损坏备份 / sync.json(0600)
│   ├── sync/               # 客户端同步引擎（后台线程 + 状态机 + 合并）
│   ├── assets.rs           # icon_assets! 自选图标集
│   ├── theme.rs            # 应用主题（ThemeConfig / 颜色 & 半径定制）
│   ├── ui/                 # 设计系统（tokens、组件：overlay/tabs/卡片按钮…）
│   └── views/              # 8 个页面
├── crates/
│   ├── sync-proto/         # 共享线协议 DTO + LWW 规则（客户端/服务端唯一真源）
│   └── server/             # axum + sqlx(sqlite) + argon2 + jwt
│       ├── src/ migrations/ tests/ Dockerfile
├── docker-compose.yml / docker-compose.tls.yml / .env.example
├── tests/                  # 集成测试：render.rs(无头渲染+截图)、sync_e2e.rs
└── docs/                   # PLAN.md / SYNC-PROTOCOL.md / DESIGN.md / DEPLOY.md
```

## 4. 阶段、泳道与门禁

> 门禁 = Gate；每阶段末跑验证 + 更新 `.slim/deepwork/courseplan-refactor/progress.md` + Oracle 评审。

### Phase 0 — 规划与协议（当前）

- 产出：本文件 + `docs/SYNC-PROTOCOL.md`
- 证据：explorer 代码地图、librarian 能力报告（已入 §2）
- **Gate 0**: Oracle 评审计划与协议（attempt 1/3）

### Phase 2.5 — 基建播种（orchestrator 直接做，接口封版）

- workspace 化、`src/lib.rs` 转换、`crates/sync-proto` 种子（DTO + LWW 谓词 + 单测）——这三样是各泳道的共同接口，由编排者冻结
- 证据：`cargo test -p course-plan-sync-proto`

### Phase 1 — 基建（4 条并行泳道，文件互斥）

| 泳道 | 内容 | 文件范围 |
| --- | --- | --- |
| S（fixer） | gpui-kit 升 0.7.3；图标资产修复（`icon_assets!`）；「图标全覆盖」测试 | `Cargo.toml`(根) `Cargo.lock` `src/main.rs` `src/assets.rs` `tests/icons.rs` |
| B1（fixer） | 服务端核心：auth + push/pull + SQLite 迁移 + 集成测试（tower oneshot） | `crates/server/{Cargo.toml,src/**,migrations/**,tests/**}` |
| B2（fixer） | 容器化：多阶段 Dockerfile、compose（+TLS 叠加）、`.env.example`、`docs/DEPLOY.md`、CI 服务端任务 | `crates/server/Dockerfile` `docker-compose*.yml` `.dockerignore` `.env.example` `docs/DEPLOY.md` `.github/workflows/**` |
| V（fixer） | 无头渲染/截图测试管线的 spike：每页渲染 + 截图 PNG + 交互冒烟；无 GPU 环境自动跳过 | `tests/render.rs` `tests/support/**` |

- 依赖：B1 依赖 sync-proto（播种已done）；V 依赖 lib.rs（播种已done）；B2 依赖冻结的二进制名/端口/环境变量契约（见 DEPLOY 契约，写入本文件 §5）
- **Gate 1**: `cargo test --workspace` + clippy + fmt + `docker build` + `compose up` + curl 健康检查 + 双客户端同步往返脚本；Oracle 评审（服务端语义/安全）；explorer 结构扫描（workspace 边界）

### Phase 2 — 客户端数据层与同步引擎（单泳道，避免 model.rs 多方写）

| 泳道 | 内容 |
| --- | --- |
| D（fixer） | model 全实体 `updated_at`/`deleted_at`（serde default 兼容旧档）+ 读取全部过滤墓碑 + 写路径收口 `Mutation API`；store：损坏文件→备份+错误不覆盖、`sync.json`(0600)；`src/sync/**` 引擎（状态机/退避/合并）+ 首登策略对话框数据；settings 临时功能版同步卡 + 顶栏状态；集成测试（进程内起 server，两个客户端 DB 收敛）+ 单测（LWW/墓碑/迁移） |

- **Gate 2**: 上述测试 + clippy；Oracle 评审（数据层与协议一致性、旧数据兼容）；explorer 复核写路径收口（无裸 `db.courses.retain` 类写操作残留）

### Phase 3 — 设计系统与外壳（designer 单泳道）

| 泳道 | 内容 |
| --- | --- |
| DS（designer） | `docs/DESIGN.md`（tokens：色板/半径/间距/字阶/动效时长与曲线；组件规格；8 页布局稿；微交互清单）+ 实现 `src/theme.rs`（自定义主题，含圆角/色彩）+ `src/ui/**` 重构（tokens、Overlay 组件、分页签/过滤器组件、空态、动效原语）+ `src/app.rs` 外壳重做（侧边栏/顶栏/同步状态） |

- 证据：`tests/render.rs` 输出全页截图（浅色+深色）；observer 视觉评审
- **Gate 3**: observer（截图 vs DESIGN.md）+ Oracle（结构与集成，attempt 1/3）

### Phase 4 — 8 视图重构（3 条 designer 泳道并行，文件互斥）

| 泳道 | 视图 |
| --- | --- |
| V1 | dashboard + checkins（灵魂页面：庆祝感动效、今日流） |
| V2 | courses + schedule + events（表单用 DatePicker/TimeField 替换裸文本） |
| V3 | reminders + memos + settings（含正式版同步卡/示例数据/数据管理） |

- 各泳道自带交互冒烟测试更新；**Gate 4**: observer 全页视觉 QA（浅/深）+ 各泳道测试；Oracle 抽查（attempt 1/3，仅风险点）

### Phase 5 — 集成、打包与收尾

- E2E：容器化后端 + 真实客户端往返（`tests/sync_e2e.rs` 可对 `COURSEPLAN_TEST_SERVER` 指向的容器运行）；`cargo build --release --locked`；README/打包说明更新；最终截图归档
- **Gate 5**: 全量验证 + 最终 Oracle 评审（整体交付）；完成后 progress.md 改写为 tombstone

## 5. 冻结接口契约（泳道间）

- **workspace**: 根 `[workspace] members = [".", "crates/sync-proto", "crates/server"]`
- **crate 名**: `course-plan`(客户端) / `course-plan-sync-proto` / `course-plan-server`
- **服务端二进制名**: `course-plan-server`；端口 `PORT=8080`；数据目录 `DATA_DIR=/data`（DB `/data/courseplan.db`）；`JWT_SECRET` 必填；`RUST_LOG` 默认 `info`
- **健康检查**: `GET /api/health` → `{"status":"ok","version":"…"}`（无需鉴权）
- **线协议**: 以 `docs/SYNC-PROTOCOL.md` 为唯一真源；DTO 类型由 `sync-proto` 提供，客户端与服务端禁止各自复制一份
- **图标**: 全部图标引用走 `src/assets.rs` 的 `AppAssets`；新增图标必须同步进该清单（有测试兜底）
- **写路径**: 客户端一切数据修改必须经过 `Database` 的 Mutation API（自动盖 `updated_at`/墓碑），视图禁止直接改字段

## 6. 验证策略（证据链）

| 层 | 手段 |
| --- | --- |
| 静态 | `cargo fmt --check`；`cargo clippy --workspace --all-targets -- -D warnings` |
| 客户端单测 | LWW/墓碑/合并、store 迁移与损坏备份、写路径时间戳 |
| 同步集成 | 进程内起真实 server（axum oneshot/temp sqlite），双客户端 DB 收敛（含删除、冲突、首登三策略、history 合并） |
| 服务端 | 端点集成测试（鉴权失败/重复注册/pull 游标/push 覆盖语义/分页上限/墓碑） |
| 容器 | `docker build` + `docker compose up` + healthcheck + 卷持久化重启 + curl 往返；`docker compose down -v` 清理 |
| UI | 无头渲染（HeadlessAppContext, lavapipe）：全页截图（浅/深）+ 关键交互冒烟（开编辑器/打卡/tab 切换）；无 GPU 环境自动跳过 |
| 设计评审 | observer 对照 DESIGN.md 看截图；发现的问题回流 designer |
| 端到端 | 容器后端 + 真实客户端引擎往返（`tests/sync_e2e.rs`） |

## 7. 主要风险与缓解

| 风险 | 缓解 |
| --- | --- |
| 时钟偏移破坏 LWW | 服务端 clamp `updated_at ≤ now+5min`；平局服务端胜（明确规则）；文档声明 |
| 旧 data.json 兼容 | 全字段 `serde(default)`；旧档 `updated_at=0` → 首登全量推（可预期）；损坏档备份不覆盖 |
| 新设备首登冲突（双方皆有数据） | 明确三选一对话框：合并(LWW, 推荐) / 云端覆盖本地 / 本地覆盖云端 |
| checkin.history 整体 LWW 丢打卡 | 客户端合并策略：history 做 map 合并（远端在平局胜），合并后 `updated_at+1` 标记待推（见协议 §7） |
| reqwest 引入 tokio → 客户端体积/打包 | rustls（去 openssl）；release `strip` 已有；体积回归抽查 |
| 无头渲染在无 GPU CI 不可用 | 渲染测试运行时探测 renderer 失败即跳过并打印提示（保证 `cargo test` 到处绿） |
| 图标清单漏加导致静默不渲染 | `tests/icons.rs` 遍历全部引用枚举，缺失即失败 |
| 并发写同文件 | 泳道文件范围互斥（§4 表）；每阶段单提交；Oracle 门禁前不并行改同文件 |

## 8. 后续可选项（不进 v1）

- 墓碑 GC（>90 天）、SSE/WebSocket 实时同步、限流、账号找回、示例数据多套模板、课表单周调课与节假日例外
