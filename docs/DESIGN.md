# CoursePlan 设计系统规范（Design Frozen）

> 状态：Phase 3（lane DS）封版。Phase 4 三个泳道（V1 dashboard + checkins；V2 courses + schedule + events；V3 reminders + memos + settings）必须**严格**按本文档落地。歧义即缺陷。
>
> 基线 commit：`d7c423a`。视觉方向：**活力儿童向**——高饱和但和谐的彩笔色调、暖纸面底、大圆角、粗壮友好控件、恰到好处的微喜悦；仍保持家长日程类工具的克制与扫描速度。

---

## 1. 设计概念与原则

### 1.1 一句话定位

> 「桌面端的儿童手账本」——家长像翻日记本一样扫课表，孩子翻到打卡页愿意停留两秒。

产品是给家长**日复一日**打开的，但儿童也是隐性用户：他们偶尔看「今天」，偶尔打卡。「活力儿童向」意味着色彩与圆角要让人心情好，但不能盖过排版密度。背景始终是**纸**（米黄/纸白），重点色（按钮、徽标、打卡庆祝）是**彩笔**（明亮糖果色），从来不是工业蓝灰那种"严肃配色"。

### 1.2 五条原则

1. **纸面先于彩笔。** 全屏背景是暖米/纸白，不是纯白也不是灰；卡片是更浅一档的纸。所有"非纸"色（主橙、强调蓝、糖粉）都**只用在需要被注意到的元素**（按钮、徽标、当日课程块、庆祝瞬间）。若一片彩色超过 5% 屏幕面积，就该退回纸色。

2. **圆角即尺度。** 全应用圆角走 `radius_lg = 14px`（按钮/输入/卡片）、`radius_md = 10px`（次级容器/章节头）、`radius_full` 用于头像/徽章/打卡勾选。圆角让控件"看起来可以被握"，是活力感的核心载体。绝不出现方形锐角（除分隔线、文字截断）。

3. **每个像素都被一个 token 拥有。** 写视图层时**不可**直接写 `px(12.)`、`theme.primary.with_alpha(0.14)` 之类的魔法数；所有间距/字号/色/圆角必须走 §2 的 token；视图层允许的例外只有两类：① 课程色（§2.6 八色调色板），② 一次性像素（如某条线 1px、进度条高度按比例计算）。

4. **动效是奖励，不是装饰。** 仅在以下三处动效：① 页面切换淡入（160ms），② 打卡勾选（spring 弹一下 + 短促光晕），③ 模态/侧滑进入（gpui-kit 自带）。其它 hover/active 用 120ms 透明度变化即可。所有动效被系统 `reduce-motion` 关闭。

5. **家长永远赢在密度。** 信息密度不低于 Phase 2 基线：7 列课表、看板 3 列、列表可滚动到 12+ 项。任何"为了活力加留白"导致家长要多翻一屏才能看到所有待办——回退。活力感来自**形状与色彩**，不是来自**留白**。

### 1.3 文案语调

- **短、口语、不端着。** 不超过 12 个汉字。
- **示例**：
  - 空态：课程管理 → "还没有课程——点右上角「新建课程」开始录入"；日程 → "本周没有额外安排"；备忘 → "记下考试范围、家长会要点、要买的文具……"
  - 成功：打卡 → "今日任务全部完成，做得不错！"；提醒 → "提醒已发送"；
  - 错误：保存失败 → "保存失败，请重试"；同步失败 → "同步未完成，下次启动会继续"。
- **绝不使用**：感叹号成串（除庆祝场景最多 1 个）、「您」字（用「你」）、emoji、生硬动词（"执行"、"初始化"等）。

---

## 2. Token 表

所有 token 落地于 `src/theme.rs` 与 `src/ui/tokens.rs`。下表是**契约**——代码与截图都必须对齐。

### 2.1 颜色：亮色（Paper Light）

纸面暖色调。`H/s/l` 是 HSL 数值（与 gpui `Hsla` 一致；`a=1.0` 省略）。

| Token | hex | Hsla | 用途 |
|---|---|---|---|
| `surface` | `#FBF6EC` | `220 0.18 0.965` | 全屏底色（**米纸**） |
| `panel` | `#FFFCF5` | `40 0.50 0.990` | 卡片/侧栏/顶栏/输入底 |
| `sidebar` | `#F5EBD9` | `38 0.42 0.915` | 侧栏纸色（比 surface 暖一档） |
| `topbar` | `#FFFCF5` | `40 0.50 0.990` | 顶栏底色（与 panel 同） |
| `border` | `#EADFC8` | `36 0.40 0.870` | 主分隔线/边框 |
| `border_strong` | `#D9C8A3` | `38 0.40 0.770` | 输入聚焦环/强分隔 |
| `foreground` | `#2A2620` | `32 0.10 0.155` | 主文字 |
| `muted_foreground` | `#7A6F60` | `32 0.10 0.420` | 次级文字/描述 |
| `muted` | `#F1E8D5` | `38 0.40 0.900` | chip/分类标签填充底 |
| `primary` | `#F08A36` | `26 0.90 0.575` | **主橙（彩笔橙）**：主要按钮、当前页指示 |
| `primary_hover` | `#E27A28` | `26 0.85 0.520` | 主橙 hover |
| `primary_active` | `#C9661E` | `26 0.80 0.450` | 主橙 active |
| `primary_foreground` | `#FFFFFF` | `0 0 1.000` | 主橙按钮文字（**白**而非黑，更活泼） |
| `secondary` | `#FFF1DD` | `34 0.80 0.940` | 次级按钮底（暖米，比 primary 弱一档） |
| `secondary_foreground` | `#5A4422` | `32 0.55 0.245` | 次级按钮文字 |
| `accent` | `#5AA9E6` | `206 0.74 0.625` | **糖蓝**：链接、信息徽标 |
| `accent_foreground` | `#FFFFFF` | `0 0 1.000` | 糖蓝文字 |
| `success` | `#5DBB6A` | `132 0.45 0.545` | 打卡完成/进度条 |
| `success_foreground` | `#FFFFFF` | `0 0 1.000` | success 文字 |
| `warning` | `#F0B73D` | `40 0.85 0.590` | 提醒徽标/进行中 |
| `warning_foreground` | `#5A4422` | `32 0.55 0.245` | warning 文字（暖深棕，在米黄上更可读） |
| `danger` | `#E36B6B` | `0 0.65 0.630` | 删除/同步错误 |
| `danger_foreground` | `#FFFFFF` | `0 0 1.000` | danger 文字 |
| `info` | `#5AA9E6` | `206 0.74 0.625` | 校内课时/普通信息（同 accent） |
| `info_foreground` | `#FFFFFF` | `0 0 1.000` | info 文字 |
| `progress_bar` | `#F08A36` | `26 0.90 0.575` | 进度条填充（= primary） |
| `ring` | `#F0B96D` | `32 0.85 0.685` | 键盘聚焦环（暖橙，不刺眼） |
| `selection` | `#FCE5C0` | `32 0.85 0.880` | 文本选区（透明橙） |
| `list_hover` | `#F1E8D5` | `38 0.40 0.900` | 列表行 hover |
| `list_active` | `#FCE5C0` | `32 0.85 0.880` | 列表行选中（透明橙） |
| `chart_1..5` | — | 见 §2.7 | 图表色阶 |

**字段映射到 `gpui_component::ThemeColor`**：`background→surface`, `foreground→foreground`, `sidebar→sidebar`, `sidebar_foreground→foreground`, `sidebar_accent→list_active`, `sidebar_accent_foreground→foreground`, `sidebar_border→border`, `primary→primary`, `primary_foreground→primary_foreground`, `secondary→secondary`, `secondary_foreground→secondary_foreground`, `muted→muted`, `muted_foreground→muted_foreground`, `accent→accent`, `accent_foreground→accent_foreground`, `danger→danger`, `danger_foreground→danger_foreground`, `success→success`, `success_foreground→success_foreground`, `warning→warning`, `warning_foreground→warning_foreground`, `info→info`, `info_foreground→info_foreground`, `border→border`, `input→border_strong`, `ring→ring`, `progress_bar→progress_bar`, `list_hover→list_hover`, `list_active→list_active`, `tab_active→surface`, `tab_active_foreground→foreground`, `selection→selection`。

### 2.2 颜色：暗色（Cozy Dark）

暖调暗色——不是纯黑/纯灰，是深棕纸 + 暖光。**任何色相都比亮色 HSL 中的 L 低 30 个百分点左右，S 上调一点以补偿**。

| Token | hex | Hsla | 用途 |
|---|---|---|---|
| `surface` | `#1B1815` | `30 0.10 0.097` | 全屏底色（深棕纸） |
| `panel` | `#26221E` | `30 0.08 0.135` | 卡片/侧栏/顶栏底 |
| `sidebar` | `#1F1B17` | `28 0.10 0.108` | 侧栏（比 surface 深一档） |
| `topbar` | `#26221E` | `30 0.08 0.135` | 顶栏底色 |
| `border` | `#3A342C` | `32 0.10 0.198` | 主分隔线 |
| `border_strong` | `#5C5240` | `34 0.18 0.310` | 聚焦环 |
| `foreground` | `#F2E9D6` | `36 0.50 0.890` | 主文字（暖米白） |
| `muted_foreground` | `#A89A82` | `36 0.18 0.580` | 次级文字 |
| `muted` | `#332D25` | `32 0.14 0.175` | chip/分类标签底 |
| `primary` | `#FFA55C` | `24 1.00 0.680` | **糖橙**（亮色版调亮以保对比度） |
| `primary_hover` | `#FFB870` | `24 1.00 0.720` | hover |
| `primary_active` | `#E27A28` | `26 0.85 0.520` | active |
| `primary_foreground` | `#1B1815` | `30 0.10 0.097` | 主按钮文字（**深棕**，与亮色相反——暗色下白字+糖橙太亮） |
| `secondary` | `#3A3328` | `32 0.18 0.193` | 次级按钮底（深棕） |
| `secondary_foreground` | `#F2E9D6` | `36 0.50 0.890` | 次级文字 |
| `accent` | `#7FBFEE` | `204 0.78 0.700` | **糖蓝（亮）** |
| `accent_foreground` | `#0F1F2E` | `210 0.50 0.120` | 深蓝底 |
| `success` | `#7CCE89` | `134 0.50 0.640` | 完成 |
| `success_foreground` | `#0E2410` | `135 0.50 0.100` | 深绿底 |
| `warning` | `#FFC664` | `38 0.95 0.690` | 提醒 |
| `warning_foreground` | `#3D2A0F` | `34 0.65 0.150` | 深棕底 |
| `danger` | `#F08B8B` | `0 0.70 0.735` | 错误（亮一档） |
| `danger_foreground` | `#2E0F0F` | `0 0.65 0.115` | 深红底 |
| `info` | `#7FBFEE` | `204 0.78 0.700` | 信息（同 accent） |
| `info_foreground` | `#0F1F2E` | `210 0.50 0.120` | 深蓝底 |
| `progress_bar` | `#FFA55C` | `24 1.00 0.680` | 进度条（= primary） |
| `ring` | `#FFC080` | `30 1.00 0.750` | 聚焦环 |
| `selection` | `#5A3D1A` | `30 0.55 0.225` | 文本选区 |
| `list_hover` | `#332D25` | `32 0.14 0.175` | 列表行 hover |
| `list_active` | `#5A3D1A` | `30 0.55 0.225` | 列表行选中 |
| `chart_1..5` | — | 见 §2.7 | 图表色阶（亮色版 +15%L） |

**暗色特别说明**：按钮文字色与亮色**完全相反**——亮色用白字 + 糖橙底，暗色用深棕字 + 亮糖橙底。这是活力儿童向的视觉承诺：**不让按钮变成悬浮糖块**。

### 2.3 圆角

写入 `Theme::radius` 与 `Theme::radius_lg`（gpui-kit 全局生效），并暴露为 `ui::tokens::radius::*`。

| Token | px | 用途 |
|---|---|---|
| `none` | 0 | 不使用（保留以备方角场景） |
| `sm` | 6 | 紧凑徽标、checkbox |
| `md` | 10 | 次级容器、章节头、状态 pill |
| `lg` | 14 | **默认**：按钮、输入框、卡片、tabs 段 |
| `xl` | 20 | 大型卡片（首屏统计块、模态容器） |
| `2xl` | 28 | 对话框/dialog/sheet |
| `3xl` | 9999 | `radius_full`——圆形头像、勾选框、徽章点 |

**全局**：亮色 `radius = 10` (md)，`radius_lg = 14` (lg)。暗色同。所有 `Button`/`Input`/`Select`/`TabBar`/`Card` 等 gpui-kit 控件自动使用。

### 2.4 间距

暴露为 `ui::tokens::space::*`。**4 的倍数**，最常用 8/12/16/24。

| Token | px | 用途 |
|---|---|---|
| `xs` | 4 | 紧贴（icon 与文字间距、checkbox 内边距） |
| `sm` | 8 | 紧凑行内（按钮内 padding-y、列表项内 chip 间距） |
| `md` | 12 | 标准按钮 padding-x、行内 gap |
| `lg` | 16 | 卡片内 padding、section 之间 gap |
| `xl` | 20 | 卡片之间、章节标题距离 |
| `2xl` | 24 | 大区块之间 |
| `3xl` | 32 | 页面外 padding（左右各 32，桌面 1280 宽度下内容 ~1216） |

### 2.5 字体与字阶

`Theme::font_family` 默认 `".SystemUIFont"`（与 Phase 1 一致）。**字阶**集中在 `ui::tokens::text::*`：

| Token | size (px) | weight | line-height | 用途 |
|---|---|---|---|---|
| `caption` | 12 | 400 | 1.4 | 辅助/占位文字 |
| `label_sm` | 13 | 500 | 1.4 | 徽标、chip |
| `body_sm` | 14 | 400 | 1.5 | 列表项副标题、卡片描述 |
| `body` | 15 | 400 | 1.5 | 默认正文（`Theme.font_size = 15`） |
| `body_emph` | 15 | 600 | 1.5 | 列表项主标题 |
| `label` | 14 | 600 | 1.4 | 按钮文字、tab 标签 |
| `subtitle` | 17 | 600 | 1.4 | 卡片标题 |
| `title` | 20 | 700 | 1.3 | 顶栏页面名 |
| `display` | 28 | 700 | 1.25 | dashboard 问候语 |
| `stat` | 32 | 700 | 1.1 | 统计卡数值 |

**说明**：`Theme.font_size = 15`（比 gpui 默认 16 略小，更紧凑——家长每天用，太大反而显呆）。中文文本行高统一 1.5；标题用 1.3 以求紧凑。

### 2.6 课程色（8 色调色板）

`ui::widgets::palette_at(index: usize) -> Hsla`，索引与 `Course.color` 对应（0-7）。**明亮糖果**但**与米黄纸面有足够对比**——不能浮夸。

| idx | name | 亮色 hex | 亮色 Hsla | 暗色 hex | 暗色 Hsla | 用途 |
|---|---|---|---|---|---|---|
| 0 | **天蓝** | `#5AA9E6` | `206 0.74 0.625` | `#7FBFEE` | `204 0.78 0.700` | 默认/语文 |
| 1 | **蜜桃** | `#F38B6B` | `14 0.85 0.685` | `#FFA38A` | `14 0.95 0.770` | 数学 |
| 2 | **薄荷** | `#5DBB6A` | `132 0.45 0.545` | `#7CCE89` | `134 0.50 0.640` | 英语 |
| 3 | **芒果** | `#F0B73D` | `40 0.85 0.590` | `#FFC664` | `38 0.95 0.690` | 补习 |
| 4 | **葡萄** | `#9D7DD6` | `264 0.55 0.665` | `#B79DE8` | `262 0.62 0.755` | 兴趣 |
| 5 | **青瓷** | `#4DBFB3` | `174 0.52 0.520` | `#74D6CB` | `174 0.58 0.640` | 体育 |
| 6 | **嫩芽** | `#A6C84B` | `76 0.55 0.535` | `#BCDB66` | `76 0.62 0.630` | 科学 |
| 7 | **玫瑰** | `#E36B95` | `340 0.65 0.655` | `#F08FB0` | `340 0.75 0.755` | 美术 |

`tint(color, theme)`：`alpha = if dark { 0.22 } else { 0.16 }`，用于课程块的填充底（足够淡以不抢主色，足够浓以让色块有"课程味道"）。

### 2.7 图表色（dashboard 课时分布）

5 阶蓝（gpui-component 的 `chart_1..5` 已有 blue ramp）。我们覆盖为：

| Token | 亮色 | 暗色 | 用途 |
|---|---|---|---|
| `chart_1` | `#A9D5F2` | `#3D7CA8` | 当日柱形（最浅） |
| `chart_2` | `#5AA9E6` | `#5AA9E6` | 普通柱形 |
| `chart_3` | `#3F8FD0` | `#7FBFEE` | 普通柱形 |
| `chart_4` | `#2E73B5` | `#A8D2F2` | 普通柱形 |
| `chart_5` | `#1F5C97` | `#CFE5F8` | 普通柱形 |
| `chart_bullish` | `#5DBB6A` | `#7CCE89` | 上升 |
| `chart_bearish` | `#E36B6B` | `#F08B8B` | 下降 |
| `chart_grid` | `#EADFC8` | `#3A342C` | 网格线 |

### 2.8 图标尺寸

| Token | px | 用途 |
|---|---|---|
| `xs` | 12 | inline（在 chip 内、紧贴文字） |
| `sm` | 14 | 默认（按钮内、tab、nav item） |
| `md` | 18 | 卡片标题旁、状态徽标 |
| `lg` | 22 | 空态/大型交互按钮 |
| `xl` | 32 | 顶栏页面图标、品牌图标 |

`Sizable` trait 的 `Size::XSmall/Small/Medium/Large` 对应 xs/sm/md/lg，xl 需手动 `.with_size(Size::Large)` + 自定义或直接 `px(32.)`。本系统默认全部使用 `Small`（=14px）。

### 2.9 阴影 / 层级

亮色：`shadow_sm = 0 1 2 hsla(0 0 0 / 0.06)`，`shadow_md = 0 4 12 hsla(0 0 0 / 0.08)`，`shadow_lg = 0 12 32 hsla(0 0 0 / 0.12)`。
暗色：`shadow_sm/md/lg` = 同样的 0 0 0 但透明度 0.30/0.40/0.50（深底阴影靠不透明度而非亮度）。

`Theme.shadow = true`（gpui-kit 组件如 Input 会画自己的 1px 阴影）。**Sheet/Dialog/Modal 用 `shadow_lg`**；浮动 popover/select 列表用 `shadow_md`；卡片 hover 时**不抬升**——用 `border_color` 由 `border` → `border_strong` 表达"被注意"。

### 2.10 动效

写入 `Theme.motion`（gpui-kit 的 `MotionTokens`）。默认覆盖：

| 维度 | 值 | 用在哪 |
|---|---|---|
| `duration_instant` | 0ms | reduce-motion 兜底 |
| `duration_fast` | **140ms** | hover/active、按钮按下反馈 |
| `duration_normal` | **180ms** | tab 切换、列表项状态切换 |
| `duration_slow` | **260ms** | 模态进出场、页面淡入 |
| `easing_enter` | `cubic-bezier(0.16, 1.0, 0.3, 1.0)` | 入场（"快出慢收"——活力感） |
| `easing_exit` | `cubic-bezier(0.4, 0, 1, 1)` | 出场 |
| `easing_move` | `cubic-bezier(0.2, 0, 0, 1)` | 平面位移（页面切换） |
| `spring_control` | `Spring::new(220ms)` 阻尼 0.85 | 打卡勾选弹一下 |
| `spring_move` | `Spring::new(320ms)` 阻尼 0.85 | 拖拽/重排 |

**reduce-motion 兜底**：所有用 `with_animation` 的组件自动遵守；自定义 spring 用 `if !reduce_motion { spring(...) } else { transition(... duration_instant) }` 包一层。**由 `theme.motion` 通过 `Presence`/`transition` 注入；Phase 4 写视图时直接调用 `ui::motion::pop_in(...)` / `ui::motion::spring_bounce(...)`。**

---

## 3. 组件规格

每个组件给出：anatomy（结构）、states（视觉态）、motion（动效）。所有 spec 都映射到具体 `ui::` 函数（见 §6 API freeze）。

### 3.1 Shell（应用外壳）

由 `src/app.rs::render` 实现。结构：`[Sidebar 256px | [Topbar 56px; Content scroll]]`，背景 `surface`，全屏满铺。

#### 3.1.1 侧栏（Sidebar）

- **Anatomy**：`brand` 块（顶部 72px 高：32×32 圆角方糖块 "课" 字 + 姓名 + 年级，下边 1px `border`）；`nav` 列表（8 项 `nav_item`，每项 36px 高、左右 padding 12、间距 2）；`footer`（底部 56px：日期 + 紧凑同步指示点）。
- **背景**：`sidebar`（米黄）。`border_r_1px border_color=border`。
- **宽度**：256px（比 Phase 2 的 216px 略宽，留足汉字+图标）。
- **Nav item 状态**：
  - default：图标 `muted_foreground`，文字 `muted_foreground`，bg `transparent`。
  - hover：bg `list_hover`，文字 `foreground`，过渡 `140ms`。
  - active：bg `list_active`（透明橙），图标 + 文字 `primary`（糖橙），左侧 3px 圆角竖条 `primary`（贴在 item 左缘内 8px）。
- **徽标**：在 `nav_item` 右侧，圆形 `rounded_full`，bg `danger`（过期提醒）或 `warning`（待打卡），文字 12px 白字。空时不渲染。
- **Footer 同步指示**：一个小圆点 6px + 一行 12px 文字（"已同步 / 未登录 / …"，详细在 §3.4）。点击 → 跳到设置页。

#### 3.1.2 顶栏（Topbar）

- **Anatomy**：`title` 块（20px 加粗页面名 + 12px 灰色副标题/统计，下边自动隐藏）；右侧 `actions`：`sync_pill` + `theme_toggle` 图标按钮 + `quick_add` 主按钮。
- **背景**：`topbar`（与 panel 同）。`border_b_1px border_color=border`。高度 56px，左右 padding 24。
- **副标题**：根据当前页生成（如 dashboard："10月17日 周六 · 今日 2 节课 · 待打卡 2 项"；courses："已建 7 门课程 · 2 项已归档"）。由 `ui::api::page_subtitle(page, app)` 提供。
- **Motion**：页面切换时整个 content 区做 180ms 淡入（opacity 0→1 + translateY(4px → 0)），尊重 reduce-motion。

#### 3.1.3 Content frame

- 外层 padding：`padding: 24`（桌面 ≥1280）/ `20`（1024-1279）。
- 默认 `flex flex_col gap: 20`。
- 顶部 `page_header`（大标题 + 描述 + 右侧操作），然后按视图自己组织。

### 3.2 按钮

API：`ui::components::button::{primary, ghost, danger, icon_only, link}`。每种都接受 `(id, label, on_click, leading_icon?, trailing_icon?)`。

| 变体 | bg | fg | border | hover | active | 高度 | padding-x |
|---|---|---|---|---|---|---|---|
| `primary` | `primary` | `primary_foreground` | — | `primary_hover` | `primary_active` | 36 | 16 |
| `secondary` | `secondary` | `secondary_foreground` | 1px `border` | `secondary` + alpha +0.06 | alpha +0.10 | 36 | 16 |
| `ghost` | transparent | `foreground` | — | `list_hover` | `list_active` | 36 | 12 |
| `danger` | `danger` | `danger_foreground` | — | alpha +0.08 | alpha +0.14 | 36 | 16 |
| `icon_only` | transparent | `muted_foreground` | — | `list_hover` | `list_active` | 32×32 | 0 |
| `link` | transparent | `accent` | — | underline | — | 24 | 4 |

- **Icon-only** 默认 hover 背景 `list_hover`、active 背景 `primary.with_alpha(0.18)`，用于顶栏切换主题、关闭等。
- 所有按钮文字 `font_size: label (14, weight 600)`；图标 14px，与文字间距 6px。
- **圆角**：`rounded_lg` (14px)。
- **Motion**：按下时 `transform: scale(0.97)` 持续 80ms；松手回弹。

### 3.3 Nav item（侧栏导航项）

API：`ui::components::nav_item`。Anatomy：`[icon 14px] [label 14/600] [badge?]`。
- 默认高度 36px，左右 padding 12px；active 左侧 3px 圆角竖条 `primary`（用 `border_l_3px` 渲染，等价视觉）。
- Badge：右上，min 18×18 圆，bg `danger`/`warning`，文字 11px 白字 600。
- 点击整行触发 on_click（不是图标）。
- test-support id：`ui:eid("nav", p.label())`（保持与 Phase 2 一致，QA 可继续使用）。

### 3.4 Sync 状态 pill

API：`ui::components::sync_pill(status: &Status)`。Anatomy：圆角胶囊 `rounded_full`，高 24px，padding-x 10px，左侧 6px 圆点（颜色随状态），右侧 12px 文字。

| 状态 | 文字 | 点颜色 | 背景 |
|---|---|---|---|
| `Idle` (已同步) | "已同步 14:30" 或 "已同步"（无 last_sync） | `success` | `success.with_alpha(0.16)` |
| `Syncing` | "同步中…" | `info`（脉动——`animate_alpha` 1.0↔0.4 1.2s loop） | `info.with_alpha(0.16)` |
| `Offline` | "离线" | `muted_foreground` | `muted` |
| `LoggedOut` | "未登录" | `muted_foreground` | `muted` |
| `Error` | "同步出错" | `danger` | `danger.with_alpha(0.16)` |

- **点击进设置页**（与设置页同步卡入口联动）。
- 顶栏使用完整 pill，侧栏 footer 使用纯文字+小点（更紧凑）。

### 3.5 Segmented tabs / Filter chips

API：`ui::components::tab_chips(items: &[(&str, T)], selected: T, on_change)`。Anatomy：水平并排 chip，外层 `rounded_lg border:border p:1` 容器；每项 `rounded_md padding-x:12 padding-y:6`。
- 默认：bg `transparent`，文字 `muted_foreground`。
- Hover：bg `list_hover`。
- Active：bg `panel`，文字 `foreground`，加 1px `border`。
- Motion：active 状态 bg 切换 140ms。
- 用于：课程管理（全部/校内/补习）、备忘（全部/已置顶）、日程筛选、未来页签分组。
- **这是 Phase 4 V2/V3 必须替换的**——视图层当前各自手写 chip 代码，统一调用此组件。

### 3.6 Tag / Badge / Chip

API：
- `ui::components::tag(text, accent: Hsla)`：圆角胶囊 22px 高，padding-x 10，bg `accent.with_alpha(0.18)`，文字 12/600 `accent`。
- `ui::components::badge(count: usize, accent: Hsla)`：圆形 18×18，圆点风格，文字 11/700 白字。
- `ui::components::chip(text, accent: Hsla)`：与 tag 视觉相似但更小（20px 高），用于 inline 标签。

差异：tag 用在卡片标题旁表示分类（"补习"/"校外"），chip 用在行内描述后（"每天 · 累计 0 次"），badge 用在导航/计数。

### 3.7 卡片（Card variants）

API：`ui::components::card(variant, title?, subtitle?, content)`，variant ∈ {`default`, `flat`, `outlined`, `accent(color)`}。

| 变体 | bg | border | 圆角 | shadow | 用途 |
|---|---|---|---|---|---|
| `default` | `panel` | 1px `border` | `lg` (14) | none | 标准卡片 |
| `flat` | `transparent` | — | — | none | 嵌入父容器（如 dashboard 时间轴） |
| `outlined` | `surface` | 2px `border` | `lg` | none | 强调分组 |
| `accent(c)` | `c.with_alpha(0.10)` | 1px `c.with_alpha(0.4)` | `lg` | none | 课程块/统计高亮 |
| `card_hero` | `panel` | 1px `border` | `xl` (20) | `shadow_md` | dashboard 顶部 4 个统计卡 |

- 标题 `subtitle` token，副标题 `body_sm` `muted_foreground`；内容区域 padding-top 12，padding-x/padding-bottom 继承卡片本身（默认 16）。
- **hover**：default 卡片 hover 时 `border_color: border_strong`，不变 shadow。
- `card_hero` 用于 dashboard 4 个统计卡，有 shadow 表达"重要数据"。

### 3.8 Stat tile

API：`ui::components::stat_tile(label, value, hint?, accent: Hsla, icon: Option<IconName>)`。用于 dashboard 顶部 4 块。
- 高度 `min_h: 108`，padding 16。
- 顶部：左侧 label `caption` `muted_foreground`，右侧 icon `accent` 18px。
- 中部：value `stat` token (32/700) `foreground`。
- 底部：hint `caption` `muted_foreground`。
- 圆角 `rounded_xl` (20)，bg `panel`，1px `border`。

### 3.9 List row

API：`ui::components::list_row(on_click?, leading?, title, subtitle?, trailing?)`。
- 高度 auto，最小 48，gap 12。
- 默认 bg `transparent`，hover bg `list_hover`，active bg `list_active`（按下），圆角 `rounded_md`。
- 用于：dashboard 今日课程时间块、课程管理/日程/提醒/备忘的列表项。

### 3.10 Progress / Streak

API：`ui::components::progress(value: f32, accent: Hsla, height: 6|10)`。
- 圆角 `rounded_full`，bg `muted` 容器，`accent` 填充；用于：checkin 进度条、staple 卡片底部条。

API：`ui::components::streak_badge(days: usize)`：火焰图标 + "连续 N 天"，文字 12/700 `warning`，图标 14px。

### 3.11 Empty state

API：`ui::components::empty_state(variant, icon, title, description?, action?)`。variants:
- `illustrated`：用于整页空态（dashboard/memos/reminders）。容器 320×240，居中，图标 56px `muted_foreground`，标题 `subtitle` `foreground`，描述 `body_sm` `muted_foreground`，底部 action 按钮。
- `compact`：用于列表内空态。容器 padding-y 24，icon 32px，标题 `body_emph`，描述 `caption`。
- `celebratory`：用于"全部完成"（checkin 全部 done）。图标 `Flame` 56px `warning`，标题更欢快，加 1 颗星 `Sparkles` 在右上 30°。
- 文案示例见 §1.3。
- Phase 4 不得自定义空态结构，必须用此组件。

### 3.12 Overlay / Dialog / Backdrop

API：
- `ui::components::overlay(content)`：全屏绝对定位 backdrop + 居中容器。backdrop `foreground.with_alpha(0.32)`（亮色）/ `0.55`（暗色）。容器 `bg: panel`、`rounded_2xl` (28)、`shadow_lg`、padding 24、min-width 420、max-width 560。**自带淡入 180ms + scale(0.96→1)**。
- `ui::components::dialog(title, body, footer)`：基于 `overlay`，顶部 title `subtitle`，底部按钮行右对齐。
- `ui::components::alert_dialog(title, description, ok_label, cancel_label, on_ok)`：基于 `Dialog` 的简化版，用于删除确认等。
- `ui::components::first_login_prompt(...)`：**专属三选一首登**——结构详见 §3.13。

### 3.13 First-login dialog（首登三选一）

保留现有 `App::first_login_prompt` 逻辑。视觉规格：
- 用 §3.12 `dialog` 包装；title "首次登录：云端与本地都有数据"。
- body 上半部分：列出云端各实体数量（"课程 × 3、课表 × 17、日程 × 2"），用 `card(variant=accent(c=accent), …)` 列出。
- body 下半部分：一段 `body_sm` 警示文字（"「本地覆盖云端」会为云端独有数据推送删除（墓碑）。"）。
- footer：3 个按钮。合并（推荐）= primary；其余两个 = secondary。
- 容器宽度 480，高度 auto。

### 3.14 Form field row

API：`ui::form::field_row(label, control)`（已存在，签名不变）。label `caption` `muted_foreground`，固定宽 84px，右边控制填满。

### 3.15 Input / Select / Date / Time

Phase 4 必须使用 gpui-kit 原生组件，**禁止**继续用 `<input type="date">` 文本：
- `Input` → `InputState` + `Input`。
- 日期 → `DatePicker`（gpui-component 自带，自带 Calendar 下拉）。
- 时间 → `TimeField`。
- 单选/多选 → `Select`（gpui-component）或现有 `ui::select::Select`（保留用于自定义内联展开）。

**统一外观**：所有输入控件边框 `border_strong`、hover `primary.with_alpha(0.5)`、focus `primary` + 2px ring `ring`；圆角 `rounded_lg`；高度 36；padding-x 12；font `body`。

### 3.16 Chart

dashboard "本周课时分布" 当前手写柱形图。Phase 4 V1 **可保留手写版本**（信息密度好），也可替换为 `chart::bar`（gpui-component）。**若保留**，颜色用 `chart_1..5`，圆角 `rounded_md` (10)；非今日柱 `chart_2.with_alpha(0.45)`，今日柱 `chart_2` 满色 + 数值 600 加粗。

### 3.17 Check-in celebration（打卡庆祝）

打卡勾选触发。Spec：
- 触发：`on_click` → 设置 done → 立刻在打卡项上 spring 一下：`scale(1 → 1.06 → 1)` 280ms ease_out_back。
- 同时勾选圆环 `success` 实心填充从 0% → 100% 在 200ms 内。
- **若当日所有打卡任务完成**：触发"庆祝瞬间"——中心 `Sparkles` icon 浮现 + scale(0 → 1) 320ms + rotate(0 → 30deg) 400ms；同时背景从 `surface` → `success.with_alpha(0.08)` 在 600ms 内淡入。1200ms 后回淡至 surface。
- **reduce-motion**：跳过所有 scale/rotate，只保留颜色变化。

### 3.18 Page transition（页面切换淡入）

content 区进入动画：opacity 0 → 1 + translateY(4px → 0)，`duration_slow` (260ms) `easing_enter`。由 `Presence` + `transition` 实现，监听 `App.page` 变化。reduce-motion 时立即显示。

---

## 4. 8 个页面的布局规格

每个页面给出：**结构**、**层级**、**关键交互**、**Phase 4 必须保留的信息密度与行为**。Phase 4 写视图时按此规范结构 + §3 组件库落地。

### 4.1 今日概览（Dashboard）——V1

- **结构**（自上而下）：
  1. Greeting 区：`display` 字号 "早上好，今天也要加油！"；副标题 `body_sm` "10月17日 周六 · 今日 2 节课 · 待打卡 2 项"。
  2. 4 个 `stat_tile` 一行：`card_hero` 圆角 20，shadow_md。
  3. 2 列网格：
     - 左：今日课程（card + 时间轴 list）
     - 右：今日打卡（card + list_row，每行 check_circle + 标题 + streak_badge）
  4. 2 列网格：
     - 左：本周课时分布（card + 手写柱形图或 chart bar）
     - 右：待办提醒（card + list_row，每行 8px 圆点 + 标题 + due/repeat + 查看全部提醒 ghost button）
  5. 2 列网格：
     - 左：快捷入口（card + 5 个 secondary 按钮：课程/计划表/日程/打卡/备忘）
     - 右：本学期概况（card + 5 个 `kv_row`：学期开始/学期结束/每周总课时/课表条目/备忘条数）
- **关键交互**：点击课程时间块 → 跳该课程对应的「计划表」或「课程」；点击打卡行 → 翻转 done；点击提醒 → set done；点击快捷入口 → 切页。
- **保留行为**：4 个统计数字（今日课程数、课程总数、今日打卡 N/M、待办提醒数）；问候语按时辰切换；本周柱形图；本学期概况 5 项。

### 4.2 课程管理（Courses）——V2

- **结构**：`page_header`（标题 "课程管理" + 描述 + 右上 `+ 新建课程` primary）；`tab_chips`（全部 / 校内 / 补习）→ 内容区。
- **内容**：课程卡片列表，每行：
  - 左侧 4px 圆角色条（按 `Course.color`）。
  - 中间：标题 + kind chip（"校内"或"补习"，info/warning）+ 状态 chip（已归档）；副标题：科目 + 老师 + 地点；底部：days summary + weekly 分钟数；备注（如有）。
  - 右侧：编辑 / 删除 action_button。
- **关键交互**：tab_chips 切换过滤；点击卡片中间区域 → 编辑器（Phase 4 V2 改为编辑模式 inline 或 modal）；编辑 / 删除按钮各自 on_click。
- **保留行为**：过滤（全部/校内/补习）、8 色调色板（按 `Course.color` 索引取）、已归档显示状态 chip、删除二次确认。

### 4.3 计划表（Schedule）——V2

- **结构**：`page_header`（标题 + 描述 "按周重复" + 右上 `+ 新增安排` primary）。下方两栏：
  - **左**：7 列周表头（周一/周二/…/周日）+ 卡片格；今天是 `today` 列高亮（border 1.5px primary + bg `primary.with_alpha(0.04)`）。
  - **右**（260px）：本周概况 card（4 个 `summary_row` 用 §3.6 tag 颜色块显示校内/补习/合计/条数）+ 安排清单 card（12 条 list_row，按 weekday+start 排序）。
- **关键交互**：点击表头 → 切换以那天为中心？**否**：保持现状，点击空白格 → 新增该 weekday；点击现有块 → 编辑；hover 整列 → 列底 `+` 按钮变 `primary`，点击新增该日。
- **保留行为**：7 列布局完整；当天列高亮；色块按课程色；右侧概况 + 安排清单；删除二次确认。

### 4.4 日程安排（Events）——V2

- **结构**：`page_header`（标题 + 右上 `+ 新建日程` primary）。下方：
  - 顶部：日期选择器（左右箭头 + 当前日期 + "今天"快捷按钮）。
  - 当日日程 list：每行 8px 圆点（按关联课程色或 primary）+ 时间段 + 标题 + 地点 + 完成 checkbox。
- **关键交互**：日期切换；点击行 → 编辑；勾选完成 → 划线 + 灰色。
- **保留行为**：日期导航；按日期筛选；已完成状态视觉；与课程关联。

### 4.5 提醒事项（Reminders）——V3

- **结构**：`page_header`（标题 + 右上 `+ 新建提醒` primary）。下方：
  - 顶部：tab_chips（待办 / 已完成，可切换 `show_done_reminders`）。
  - 提醒列表：每行 8px 圆点（`Once` 提醒 = warning，循环提醒 = info）+ 标题 + 内容预览 + due_at + repeat label + 编辑 / 删除。
  - 底部空白区可放已完成列表（当 `show_done=true`）。
- **关键交互**：切换待办/已完成；勾选完成；编辑；删除。
- **保留行为**：到期高亮（due 在过去但未完成 → row 左侧加 warning 边）；重复类型标签；本地通知（`start_timer` 不动）。

### 4.6 打卡任务（Check-Ins）——V1

- **结构**：`page_header`（标题 + 右上 `+ 新建任务` primary）。下方：
  - 顶部：今日完成率 progress 条（0% 文字 + 条 + N/M 数字）。
  - 3 列看板：`待打卡`（今日 active 未 done）/ `今日已完成`（今日 active done）/ `全部任务`（未归档）。每列：
    - 列头：标题 + 计数。
    - 任务卡片：check_circle + 标题 + 描述 + repeat chip（"每天"等）+ weekday chip + streak + 编辑/归档 action_button。
- **关键交互**：勾选打卡 → 触发庆祝动效；编辑；归档（移到归档列表，下次不再出现）。**归档需二次确认**（AlertDialog）。
- **保留行为**：3 列看板；streak 计算；今日 active 逻辑；repeat/weekday 表达；归档二次确认。

### 4.7 备忘笔记（Memos）——V3

- **结构**：`page_header`（标题 + 右上 `+ 新建备忘` primary）。下方：
  - `tab_chips`（全部 / 已置顶）。
  - 3 列网格（`grid_cols(3)`）：每张备忘卡：左侧 4px 角色条（按 `Memo.color`）+ 标题 + 内容（多行省略）+ 更新时间 + 置顶 chip（pinned=true 时）+ 编辑/置顶/删除。
- **关键交互**：切换 tab；点击编辑；切换置顶；删除。
- **保留行为**：置顶置顶；按 pinned + updated_at 排序；删除二次确认。

### 4.8 设置（Settings）——V3

- **结构**：单列卡片堆叠（间距 16）：
  1. **学生信息** card：姓名/年级/学期开始/学期结束表单。
  2. **提醒与启动** card：reminder_enabled Switch + auto_load_today Switch。
  3. **外观主题** card：浅色/深色 tab_chips。
  4. **同步** card（保持现有 sync_card 逻辑——Phase 4 V3 改样式，**不改行为**）。
  5. **数据存储** card：数据文件路径 + 各实体计数 kv_rows。
  6. **载入示例数据** / **清空全部数据** 危险操作 secondary/danger 按钮（含二次确认）。
- **关键交互**：所有输入直接 commit；Switch 翻转 commit；主题切换即时。
- **保留行为**：所有现有逻辑不动；同步卡片的所有现有代码原样。

---

## 5. 可访问性 / 暗色指南 / 通用规则

### 5.1 对比度（WCAG AA）

- 普通文字 `foreground` on `surface/panel`：≥7:1（亮色） / ≥8:1（暗色）。
- `muted_foreground` on `surface`：≥4.5:1（亮色） / ≥5:1（暗色）。
- 按钮文字 `primary_foreground` on `primary`：≥4.5:1（亮色 + 亮色下浮点数都满足）。
- `warning_foreground`（暖棕）在 `warning`（芒果黄）底上：⚠️ 4.0:1 临界，**仅用于大字号（≥18px）或加粗**。在 chip/徽标中使用 12/600 + 足够 padding 来保证实际命中区域不模糊。

### 5.2 触控目标

桌面端鼠标交互，但**儿童偶尔用**且未来可能触屏：
- 所有可点击元素 hit area ≥ 32×32。
- 按钮默认 36px 高；icon-only 按钮 32×32；nav item 36px；tab_chips 30px（密集场景下可降到 30，但绝不低于 28）。

### 5.3 键盘焦点

- `Theme.focus_ring = true`，颜色 `ring`（暖橙）。
- 凡焦点可达元素必须有 ring：按钮、input、select、tab、nav item。
- 减少依赖鼠标：nav item 必须可 Tab 切换 + Enter 触发。

### 5.4 暗色指南

- 文字颜色**不变黑**，永远是暖米白（`#F2E9D6` 区间）。
- 主色提亮 ~15% L 以保证在深底上仍饱和。
- **禁用 emoji 表情**做状态指示——用 chip/标签/图标。
- 阴影透明度上调，弥补深底"无深可叠"的问题。

### 5.5 减少动效 (`reduce_motion`)

- 所有 spring/scale/rotate → 立即完成（`duration_instant`）。
- 颜色和透明度过渡保留（信息性）。
- 由 `theme.motion` 自动遵守 + 自定义 spring 调用前 `if !reduce_motion { spring() } else { transition() }`。

---

## 6. 组件 API Freeze（Phase 4 必须遵守）

Phase 4 三条 lane 只能使用以下公开 API。**签名与命名是契约**——任何偏离需要回到本文件更新签名再改代码。

### 6.1 `src/theme.rs`

- `pub fn init(cx: &mut App)`：装载应用主题（调一次，幂等）。可在 `lib.rs::run()` 和 `tests/support::Harness::new()` 中调用。
  - 内部会把 CoursePlan `ThemeConfig` 装入 `theme.light_theme` / `theme.dark_theme`，让任何 `Theme::change(mode)` 加载我们的配置而不是 gpui-component 默认；并调一次 `apply_for_mode` 写 `theme.colors`。
- `pub fn apply_for_mode(cx: &mut App)`：直接把 `colors / radius / font / motion` 写入 Theme 全局——`Theme::change` 之后必须再调一次（或依赖 `init` 后的副作用）以保证 `theme.colors` 是 CoursePlan 调色板。测试 harness 与 `toggle_mode` 都显式调用。
- `pub fn toggle_mode(window: Option<&mut Window>, cx: &mut App)`：切换 ThemeMode（封装 `Theme::change` + `apply_for_mode`）。

### 6.2 `src/ui/mod.rs`

```
pub mod tokens;
pub mod palette;
pub mod motion;
pub mod components;
pub mod form;
pub mod select;
pub mod widgets;  // 兼容层（薄壳）
```

### 6.3 `src/ui/tokens.rs`

```
// 颜色（直接 re-export 主题字段 + 派生别名）
pub mod color {
    pub const SURFACE: ...; pub const PANEL: ...;
    // 这些是**值**，不是函数；Phase 4 直接引用
    // 推荐写法：cx.theme().surface / cx.theme().primary，避免硬编码
}

// 尺寸
pub const SPACE_XS: Pixels = px(4.);
pub const SPACE_SM: Pixels = px(8.);
pub const SPACE_MD: Pixels = px(12.);
pub const SPACE_LG: Pixels = px(16.);
pub const SPACE_XL: Pixels = px(20.);
pub const SPACE_2XL: Pixels = px(24.);
pub const SPACE_3XL: Pixels = px(32.);

// 圆角
pub const RADIUS_SM: Pixels = px(6.);
pub const RADIUS_MD: Pixels = px(10.);
pub const RADIUS_LG: Pixels = px(14.);
pub const RADIUS_XL: Pixels = px(20.);
pub const RADIUS_2XL: Pixels = px(28.);
pub const RADIUS_FULL: Pixels = px(9999.);

// 图标尺寸
pub const ICON_XS: Pixels = px(12.);
pub const ICON_SM: Pixels = px(14.);
pub const ICON_MD: Pixels = px(18.);
pub const ICON_LG: Pixels = px(22.);
pub const ICON_XL: Pixels = px(32.);

// 高度
pub const BUTTON_H: Pixels = px(36.);
pub const ICON_BUTTON_H: Pixels = px(32.);
pub const INPUT_H: Pixels = px(36.);
pub const NAV_ITEM_H: Pixels = px(36.);
pub const TOPBAR_H: Pixels = px(56.);
pub const SIDEBAR_W: Pixels = px(256.);
```

### 6.4 `src/ui/palette.rs`

- `pub const COURSE_PALETTE: [Hsla; 8]`：8 色调色板（§2.6）。
- `pub fn palette_at(index: usize) -> Hsla`：取索引对应色。
- `pub fn tint(color: Hsla, theme: &Theme) -> Hsla`：填充淡底。

### 6.5 `src/ui/motion.rs`

- `pub fn pop_in(id: impl Into<ElementId>, window: &mut Window, cx: &mut App) -> Transition`：进入（透明度 0→1 + Y 位移）。
- `pub fn pop_out(...)`：退出。
- `pub fn spring_bounce(window: &mut Window, cx: &mut App) -> Spring`：庆祝弹簧。
- `pub fn page_enter_animation(...)`：页面切换。

### 6.6 `src/ui/components/` 模块（每个 .rs 一个组件族）

#### `button.rs`

```
pub fn primary(id: ElementId, label: impl Into<SharedString>, on_click: impl Fn(...) + 'static) -> Button;
pub fn primary_with_icon(id, label, icon: IconName, on_click) -> Button;
pub fn secondary(id, label, on_click) -> Button;
pub fn secondary_with_icon(id, label, icon, on_click) -> Button;
pub fn ghost(id, label, on_click) -> Button;
pub fn ghost_with_icon(id, label, icon, on_click) -> Button;
pub fn danger(id, label, on_click) -> Button;
pub fn danger_with_icon(id, label, icon, on_click) -> Button;
pub fn icon_only(id, icon: IconName, tooltip: impl Into<SharedString>, on_click) -> Button;
pub fn link(id, label, on_click) -> Button;
```

实现细节：内部走 `gpui_kit::component::button::Button::new(id).primary()/secondary()/...compact().label(...).icon(...).on_click(...)`。每个变体强制注入正确颜色（确保不漏 `.text_color`）。

#### `nav_item.rs`

```
pub fn nav_item(id: ElementId, icon: IconName, label: impl Into<SharedString>, active: bool, badge: Option<usize>, on_click) -> impl IntoElement;
```

#### `sync_pill.rs`

```
pub fn sync_pill(status: &Status, on_click: impl Fn(...) + 'static) -> impl IntoElement;
pub fn sync_indicator_compact(status: &Status) -> impl IntoElement;  // 侧栏 footer 用
```

#### `tab_chips.rs`

```
pub fn tab_chips<T: Copy + PartialEq + 'static>(id_prefix: &str, items: Vec<(&str, T)>, selected: T, on_change: impl Fn(T, &mut Window, &mut App) + 'static) -> impl IntoElement;
```

#### `tag.rs`

```
pub fn tag(text: impl Into<SharedString>, accent: Hsla) -> impl IntoElement;
pub fn badge(count: usize, accent: Hsla) -> impl IntoElement;
pub fn chip(text: impl Into<SharedString>, accent: Hsla) -> impl IntoElement;
```

#### `card.rs`

```
pub enum CardVariant { Default, Flat, Outlined, Accent(Hsla), Hero }
pub fn card(variant: CardVariant, title: Option<impl Into<SharedString>>, subtitle: Option<impl Into<SharedString>>, content: impl IntoElement) -> impl IntoElement;
```

**Back-compat shim**：`pub fn card(theme: &Theme, title, subtitle, content) -> impl IntoElement`（保留旧签名给 views 直接调用，内部映射到 `CardVariant::Default`）。

#### `stat_tile.rs`

```
pub fn stat_tile(label, value, hint: Option<...>, accent: Hsla, icon: Option<IconName>) -> impl IntoElement;
```

#### `list_row.rs`

```
pub fn list_row(on_click: Option<impl Fn(...) + 'static>, leading: Option<impl IntoElement>, title: impl Into<SharedString>, subtitle: Option<...>, trailing: Option<impl IntoElement>) -> impl IntoElement;
```

#### `progress.rs`

```
pub fn progress(value: f32, accent: Hsla, height: Pixels /* 6 or 10 */) -> impl IntoElement;
pub fn streak_badge(days: usize) -> impl IntoElement;
```

#### `empty_state.rs`

```
pub enum EmptyVariant { Illustrated, Compact, Celebratory }
pub fn empty_state(variant: EmptyVariant, icon: IconName, title: impl Into<SharedString>, description: Option<...>, action: Option<impl IntoElement>) -> impl IntoElement;
```

**Back-compat shim**：`pub fn empty_state(theme: &Theme, icon: IconName, title, description: Option<...>) -> impl IntoElement`（保留旧签名）。

#### `overlay.rs`

```
pub fn overlay(content: impl IntoElement) -> impl IntoElement;
pub fn dialog(title: impl Into<SharedString>, body: impl IntoElement, footer: Option<impl IntoElement>) -> impl IntoElement;
pub fn alert_dialog(title, description, ok_label, cancel_label, on_ok: impl Fn(...) + 'static) -> impl IntoElement;
pub fn first_login_prompt(round_id: u64, probe: &ProbeSummary, on_choice: impl Fn(FirstLoginPolicyKind, &mut Window, &mut App) + 'static) -> impl IntoElement;
```

**Back-compat shim**：`pub fn section_title(theme: &Theme, text, trailing) -> impl IntoElement`。

#### `kv_row.rs`

```
pub fn kv_row(key, value) -> impl IntoElement;
```

#### `check_circle.rs`

```
pub fn check_circle(done: bool, size: Pixels) -> impl IntoElement;
```

#### `page_header.rs`

```
pub fn page_header(title: impl Into<SharedString>, subtitle: Option<...>, actions: Option<impl IntoElement>) -> impl IntoElement;
pub fn page_subtitle(page: Page, db: &Database) -> SharedString;
```

### 6.7 `src/ui/widgets.rs`（Back-compat 兼容层）

保留以下**薄壳包装**，views 可继续调用，内部调用 §6.6 新组件：

```
pub const PALETTE: [Hsla; 8] = palette::COURSE_PALETTE;
pub fn palette_at(index) -> Hsla;
pub fn surface(theme) -> Hsla;       // = theme.surface
pub fn panel(theme) -> Hsla;          // = theme.panel
pub fn tint(color, theme) -> Hsla;    // = palette::tint
pub fn eid(prefix, key) -> ElementId;
pub fn close_editor(app, cx);         // 必备，测试依赖
pub fn action_button(id, label, on_click);  // = components::button::ghost
pub fn section_title(theme, text, trailing);  // = components::page_header::section_title
pub fn card(theme, title, subtitle, content);  // = components::card::card(Default, ...)
pub fn stat_tile(theme, label, value, hint, accent, icon);  // = components::stat_tile
pub fn empty_state(theme, icon, title, desc);  // = components::empty_state(Compact, ...)
pub fn chip(text, color, filled);    // = components::chip
pub fn kv_row(theme, key, value);
pub fn check_circle(theme, done, size);  // = components::check_circle

pub struct Skin { ... }  // 保持；views 不直接用，但 app.rs 仍可用
```

`Skin` 字段命名不变：`surface/panel/sidebar/border/fg/muted/primary/primary_fg`，对应 `theme.{surface,panel,sidebar,border,foreground,muted_foreground,primary,primary_foreground}`。

### 6.8 测试支持 IDs（QA/Phase 4 用）

shell 元素使用以下 id（必须稳定）：

```
sidebar items:    "ui:eid(\"nav\", {page_label})"
brand block:      "ui:eid(\"brand\", \"\")"            // 整块
theme toggle:     "ui:eid(\"topbar\", \"theme-toggle\")"
quick add:        "ui:eid(\"topbar\", \"quick-add\")"
sync pill:        "ui:eid(\"topbar\", \"sync-pill\")"
sync pill comp:   "ui:eid(\"sidebar\", \"sync-indicator\")"
first-login ok:   "ui:eid(\"first-login\", \"ok\")"
first-login cancel:"ui:eid(\"first-login\", \"cancel\")"
```

### 6.9 图标注册（写入 `src/assets.rs`）

下列新增图标必须加入 `APP_ICONS` 与 `icon_assets!` 块（如不在默认集中）：
- `Sparkles`（庆祝用）
- `LogIn` / `LogOut`（同步状态可选）
- `MoreHorizontal`（行尾更多操作）
- `CircleCheck` / `CircleCheckBig`（已完成标记，可与现有 `Check` 复用）
- `CircleDashed`（未完成标记）
- `Pencil`（编辑备用，目前用 `action_button` 文字"编辑"）

`tests/icons.rs` 会扫描源码 → 加图标前先加 APP_ICONS；漏一个即失败。

---

## 7. 已冻结的"做什么 / 不做什么"

**做**：
- 提供 paper-warm 配色与圆角尺度的全局 Theme，所有 gpui-kit 组件自动受益。
- 重做侧栏（256px、活 logo、nav 项状态、徽标、紧凑同步指示）。
- 重做顶栏（页面名+副标题、sync pill、theme toggle、quick add）。
- 重做 first-login dialog（保留三选一逻辑）。
- 提供 §6 公开 API + 兼容 shim，**视图代码无需修改即可通过编译与现有截图测试**。
- 加 shell 元素 test-support id（不触及视图）。

**不做**（留给 Phase 4）：
- 不重写 8 个 view 的内部结构。
- 不改 sync settings card 的逻辑（仅样式）。
- 不替换现有手写表单输入——Phase 4 V2 改为 gpui-kit DatePicker/TimeField/Input。
- 不删 chip 代码（保留以备 Phase 4 V2/V3 切换到 tab_chips）。
- 不改 store、model、sync 任何一行。

---

## 8. 验证清单（Gate 3 证据）

- [ ] `target/visual/*.png` 8 页 × 浅/深 = 16 张 + 至少 1 张交互（smoke-course-editor）全部刷新。
- [ ] `cargo test --workspace` 全绿（86 个测试，含 tests/icons.rs 自检通过；其中 theme::tests 8 个：调色板构造、motion 范围、hex round-trip、Hair Light/Dark 直接 `Theme::change` 不丢色、toggle_mode 往返、init 不回退 gpui 默认）。
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 无 warning。
- [ ] `cargo fmt --all --check` 无 diff。
- [ ] 切换主题：顶栏 toggle 按钮点击后浅↔深色生效，所有组件随之刷新（视觉证据：dashboard-light.png vs dashboard-dark.png 明显不同）。
- [ ] Sidebar nav 点击切换：截图前后应有差异（Phase 4 自动覆盖；Phase 3 仅保证 shell 渲染稳定）。
- [ ] First-login dialog：模拟 `first_login_prompt` 出现时，样式为暖纸 + 三按钮，不破逻辑。

---

## 9. 版本与变更

| 版本 | 日期 | 变更 |
|---|---|---|
| v1.0 | 2026-10-17 | Phase 3 封版（lane DS 产出） |
| v1.1 | 2026-10-17 | Gate 3 预检修复：色相统一走 `hsl_deg(度数)`（gpui `Hsla.h ∈ [0,1)`）；暗色「嫩芽」hex 校正 `#BCDC72 → #BCDB66`（与 HSL 76/0.62/0.630 一致）；`Theme::change` 不再覆盖自有调色板（安装自定义 ThemeConfig） |

任何后续调整必须更新本表并同步 §6 API Freeze。