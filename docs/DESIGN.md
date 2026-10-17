# CoursePlan 设计系统规范（Design Frozen）

> 状态：**设计冻结（Design Frozen）**。本规范 v1.4 已随 **Phase 4 完整交付**——8 个页面（今日概览 / 课程管理 / 计划表 / 日程 / 提醒 / 打卡 / 备忘 / 设置）均按本文档落地。本文档是视图层的唯一真源，**§6 API Freeze 与 §9 版本与变更为权威**，任何偏离即缺陷。
>
> 基线 commit：`4b5a550`（Phase 5 收尾；含 Phase 4 全部视图与 v1.4 对齐）。视觉方向：**活力儿童向**——高饱和但和谐的彩笔色调、暖纸面底、大圆角、粗壮友好控件、恰到好处的微喜悦；仍保持家长日程类工具的克制与扫描速度。
>
> **顶栏第一规则（Phase 4 起长期有效）**：页面名、副标题与主 CTA 只属于顶栏；内容区不重复页面名、不放第二个同义主按钮（详见 §3.1.4，§4 开头引用并落到每一页）。

---

## 1. 设计概念与原则

### 1.1 一句话定位

> 「桌面端的儿童手账本」——家长像翻日记本一样扫课表，孩子翻到打卡页愿意停留两秒。

产品是给家长**日复一日**打开的，但儿童也是隐性用户：他们偶尔看「今天」，偶尔打卡。「活力儿童向」意味着色彩与圆角要让人心情好，但不能盖过排版密度。背景始终是**纸**（米黄/纸白），重点色（按钮、徽标、打卡庆祝）是**彩笔**（明亮糖果色），从来不是工业蓝灰那种"严肃配色"。

### 1.2 五条原则

1. **纸面先于彩笔。** 全屏背景是暖米/纸白，不是纯白也不是灰；卡片是更浅一档的纸。所有"非纸"色（主橙、强调蓝、糖粉）都**只用在需要被注意到的元素**（按钮、徽标、当日课程块、庆祝瞬间）。若一片彩色超过 5% 屏幕面积，就该退回纸色。

2. **圆角即尺度。** 全应用圆角走 `radius_lg = 14px`（按钮/输入/卡片）、`radius_md = 10px`（次级容器/章节头）、`radius_full` 用于头像/徽章/打卡勾选。圆角让控件"看起来可以被握"，是活力感的核心载体。绝不出现方形锐角（除分隔线、文字截断）。

3. **每个像素都被一个 token 拥有。** 写视图层时**不可**直接写 `px(12.)`、`theme.primary.with_alpha(0.14)` 之类的魔法数；所有间距/字号/色/圆角必须走 §2 的 token；视图层允许的例外只有两类：① 课程色（§2.6 八色调色板），② 一次性像素（如某条线 1px、进度条高度按比例计算）。

4. **动效是奖励，不是装饰。** 仅在以下三处动效：① 页面切换淡入（260ms，§3.18），② 打卡勾选（spring 弹一下 + 短促光晕），③ 模态/侧滑进入（gpui-kit 自带）。其它 hover/active 用 120ms 透明度变化即可。所有动效被系统 `reduce-motion` 关闭。

5. **家长永远赢在密度。** 信息密度不低于 Phase 2 基线：7 列课表、看板 3 列、列表可滚动到 12+ 项。任何"为了活力加留白"导致家长要多翻一屏才能看到所有待办——回退。活力感来自**形状与色彩**，不是来自**留白**。

### 1.3 文案语调

- **短、口语、不端着。** 不超过 12 个汉字。
- **示例**：
  - 空态：课程管理 → "还没有课程——点右上角「快速添加」开始录入"；日程 → "本周没有额外安排"；备忘 → "记下考试范围、家长会要点、要买的文具……"
  - 成功：打卡 → "今日任务全部完成，做得不错！"；提醒 → "提醒已发送"；
  - 错误：保存失败 → "保存失败，请重试"；同步失败 → "同步未完成，下次启动会继续"。
- **绝不使用**：感叹号成串（除庆祝场景最多 1 个）、「您」字（用「你」）、emoji、生硬动词（"执行"、"初始化"等）。

---

## 2. Token 表

所有 token 落地于 `src/theme.rs` 与 `src/ui/tokens.rs`。下表是**契约**——代码与截图都必须对齐。

### 2.1 颜色：亮色（Paper Light）

纸面暖色调。**hex 是契约**——代码（`src/theme.rs::light_palette()`）通过
`hex_color("#…")` 反算 Hsla；测试断言 `hex_color(palette_token) == 设计表 hex`（±2 通道）。
**饱和 fill（success / warning / danger / accent）的加深档位**保证白字 WCAG AA ≥4.5
（详见 §5.1）；primary 保留 `#F08A36` 但文字用深棕 ink `#1F1612`（7.10:1 AAA）。

| Token | hex | 用途 |
|---|---|---|
| `surface` | `#FBF6EC` | 全屏底色（**米纸**） |
| `panel` | `#FFFCF5` | 卡片/侧栏/顶栏/输入底 |
| `sidebar` | `#F5EBD9` | 侧栏纸色（比 surface 暖一档） |
| `topbar` | `#FFFCF5` | 顶栏底色（与 panel 同） |
| `border` | `#EADFC8` | 主分隔线/边框 |
| `border_strong` | `#D9C8A3` | 输入边框/强分隔 |
| `foreground` | `#2A2620` | 主文字 |
| `muted_foreground` | `#726758` | 次级文字/描述（sidebar 4.68 / surface 5.14 / panel 5.40） |
| `muted` | `#F1E8D5` | chip/分类标签填充底 |
| `primary` | `#F08A36` | **主橙（彩笔橙）**：主要按钮、当前页指示 |
| `primary_hover` | `#E27A28` | hover |
| `primary_active` | `#C9661E` | active |
| `primary_foreground` | `#1F1612` | 主橙按钮文字（深棕 ink，7.10:1） |
| `secondary` | `#FFF1DD` | 次级按钮底（暖米） |
| `secondary_foreground` | `#5A4422` | 次级按钮文字 |
| `accent` | `#3478AC` | **糖蓝（加深档）**：信息徽标、填充底（白字 4.74:1） |
| `accent_foreground` | `#FFFFFF` | 糖蓝文字 |
| `link` | `#2E6A99` | 正文链接（panel 5.63:1 / surface 5.36:1） |
| `link_hover` | `#2A5F8A` | 链接 hover（panel 6.61:1） |
| `link_active` | `#24537A` | 链接 active（panel 7.90:1） |
| `success` | `#3D8246` | 完成（加深档，白字 4.69:1） |
| `success_foreground` | `#FFFFFF` | success 文字 |
| `warning` | `#9A650F` | 提醒（加深档，白字 4.95:1） |
| `warning_foreground` | `#FFFFFF` | warning 文字 |
| `danger` | `#9C2929` | 删除/同步错误（加深档，白字 7.60:1） |
| `danger_foreground` | `#FFFFFF` | danger 文字 |
| `info` | `#3478AC` | 校内课时/普通信息（同 accent） |
| `info_foreground` | `#FFFFFF` | info 文字 |
| `progress_bar` | `#F08A36` | 进度条填充（= primary） |
| `ring` | `#F0B96D` | 键盘聚焦环（暖橙，不刺眼） |
| `selection` | `#F08A363D` | 文本选区（primary α0.24） |
| `list_hover` | `#F1E8D5` | 列表行 hover |
| `list_active` | `#F08A3633` | 列表行选中（primary α0.20） |
| `chart_1..5` | 见 §2.7 | 图表色阶 |

> HSL 是运行期派生量（`hex_color()` 反算），**不是契约**；上表的 hex 才是。
> `selection` / `list_active` 保留 alpha：vendor `apply_config` 会对这两个字段
> 做 α 上限（0.30 / 0.20），取上限内可直接 round-trip。
>
> **链接色不是 primary**：`button::link` 与正文链接一律用 `link` 家族
> （`button::link` 的 fg 来自 `Theme::link/link_hover/link_active`）。亮色
> `primary` 在 panel 上只有 2.4:1，不能当链接文字；`link` 从 `accent` 再加深一档，
> 保证 panel / surface 都 ≥4.5。


**字段映射到 `gpui_component::ThemeColor`**：`background→surface`, `foreground→foreground`, `sidebar→sidebar`, `sidebar_foreground→foreground`, `sidebar_accent→list_active`, `sidebar_accent_foreground→foreground`, `sidebar_border→border`, `primary→primary`, `primary_foreground→primary_foreground`, `secondary→secondary`, `secondary_foreground→secondary_foreground`, `muted→muted`, `muted_foreground→muted_foreground`, `accent→accent`, `accent_foreground→accent_foreground`, `link→link`, `link_hover→link_hover`, `link_active→link_active`, `danger→danger`, `danger_foreground→danger_foreground`, `success→success`, `success_foreground→success_foreground`, `warning→warning`, `warning_foreground→warning_foreground`, `info→info`, `info_foreground→info_foreground`, `border→border`, `input→border_strong`, `ring→ring`, `progress_bar→progress_bar`, `list_hover→list_hover`, `list_active→list_active`, `tab_active→surface`, `tab_active_foreground→foreground`, `selection→selection`。

### 2.2 颜色：暗色（Cozy Dark）

暖调暗色——不是纯黑/纯灰，是深棕纸 + 暖光。**hex 是契约**——代码
（`src/theme.rs::dark_palette()`）通过 `hex_color("#…")` 反算 Hsla；测试断言
`hex_color(palette_token) == 设计表 hex`（±2 通道）。详见 §5.1 对比度表。

| Token | hex | 用途 |
|---|---|---|
| `surface` | `#1B1815` | 全屏底色（深棕纸） |
| `panel` | `#26221E` | 卡片/侧栏/顶栏底 |
| `sidebar` | `#1F1B17` | 侧栏（比 surface 深一档） |
| `topbar` | `#26221E` | 顶栏底色 |
| `border` | `#3A342C` | 主分隔线 |
| `border_strong` | `#5C5240` | 输入边框 |
| `foreground` | `#F2E9D6` | 主文字（暖米白，≥13:1） |
| `muted_foreground` | `#A89A82` | 次级文字（≥5.7:1） |
| `muted` | `#332D25` | chip/分类标签底 |
| `primary` | `#FFA55C` | **糖橙**（亮色版调亮；深棕字 9.08:1） |
| `primary_hover` | `#FFB870` | hover |
| `primary_active` | `#E27A28` | active |
| `primary_foreground` | `#1B1815` | 主按钮文字（**深棕**） |
| `secondary` | `#3A3328` | 次级按钮底（深棕） |
| `secondary_foreground` | `#F2E9D6` | 次级文字 |
| `accent` | `#7FBFEE` | **糖蓝（亮）**（深蓝字 8.43:1） |
| `accent_foreground` | `#0F1F2E` | 深蓝底 |
| `link` | `#7FBFEE` | 正文链接（panel 7.96:1 / surface 8.91:1） |
| `link_hover` | `#97CBF3` | 链接 hover（panel 9.14:1） |
| `link_active` | `#6AABE0` | 链接 active（panel 6.40:1） |
| `success` | `#7CCE89` | 完成（深绿字 8.64:1） |
| `success_foreground` | `#0E2410` | 深绿底 |
| `warning` | `#FFC664` | 提醒（深棕字 8.80:1） |
| `warning_foreground` | `#3D2A0F` | 深棕底 |
| `danger` | `#F08B8B` | 错误（亮一档；深红字 7.36:1） |
| `danger_foreground` | `#2E0F0F` | 深红底 |
| `info` | `#7FBFEE` | 信息（同 accent） |
| `info_foreground` | `#0F1F2E` | 深蓝底 |
| `progress_bar` | `#FFA55C` | 进度条（= primary） |
| `ring` | `#FFC080` | 聚焦环 |
| `selection` | `#5A3D1A4D` | 文本选区（深棕 α0.30） |
| `list_hover` | `#332D25` | 列表行 hover |
| `list_active` | `#5A3D1A33` | 列表行选中（深棕 α0.20） |
| `chart_1..5` | 见 §2.7 | 图表色阶 |


**暗色特别说明**：按钮文字色与亮色**相反**——亮色主橙用深棕 ink（7.10:1），暗色亮糖橙底用深棕字（9.08:1）；其它饱和 fill 亮色用"加深档 + 白字"、暗色用"提亮档 + 深色字"。这是活力儿童向的视觉承诺：**不让按钮变成悬浮糖块，也不让白字落在浅黄上**。链接在暗色直接用提亮的 `accent`（`#7FBFEE`），不需要再加深。

### 2.3 圆角

写入 `Theme::radius` 与 `Theme::radius_lg`（gpui-kit 全局生效），并暴露为 `ui::tokens::{RADIUS_*}` 顶层常量（**没有** `tokens::radius` 子模块）。

| Token | px | 用途 |
|---|---|---|
| `RADIUS_SM` | 6 | 紧凑徽标、checkbox、链接按钮 |
| `RADIUS_MD` | 10 | 次级容器、章节头、状态 pill、icon-only 按钮 |
| `RADIUS_LG` | 14 | 按钮、卡片、输入框、tabs 段 |
| `RADIUS_XL` | 20 | 大型卡片（首屏统计块） |
| `RADIUS_2XL` | 28 | 对话框/dialog/sheet |
| `RADIUS_FULL` | 9999 | 圆形头像、勾选框、徽章点 |

**全局**：`Theme.radius = 10`（gpui-kit 默认控件，如 Input/Select/TabBar）、`Theme.radius_lg = 14`（对话框等大容器）。应用自绘的按钮/卡片在组件里显式用 `RADIUS_LG`(14)，所以视觉上"默认圆角"是 14。

### 2.4 间距

暴露为 `ui::tokens::{SPACE_*}` 顶层常量（**没有** `tokens::space` 子模块）。**4 的倍数**，最常用 8/12/16/24。

| Token | px | 用途 |
|---|---|---|
| `xs` | 4 | 紧贴（icon 与文字间距、checkbox 内边距） |
| `sm` | 8 | 紧凑行内（按钮内 padding-y、列表项内 chip 间距） |
| `md` | 12 | 标准按钮 padding-x、行内 gap |
| `lg` | 16 | 卡片内 padding、section 之间 gap |
| `xl` | 20 | 卡片之间、章节标题距离、**页面 padding（`p_5`，§3.1.3）** |
| `2xl` | 24 | 大区块之间 |
| `3xl` | 32 | 备用大间距（当前页面未用） |

### 2.5 字体与字阶

`Theme::font_family` 默认 `".SystemUIFont"`（与 Phase 1 一致）。**字号**集中在 `ui::tokens::text::*`（`pub mod text`，只有 size 常量；字重/行高由组件显式设置，颜色永远取 `&Theme`——**没有** `tokens::color`）：

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

`ui::palette::palette_at_for(index: usize, theme: &Theme) -> Hsla`，索引与 `Course.color` 对应（0-7）。**Phase 4 必须使用此函数**（暗色主题会切到暗色盘；`palette_at(index)` 仅保留给无主题的兼容调用）。**明亮糖果**但**与米黄纸面有足够对比**——不能浮夸。

| idx | name | 亮色 hex | 暗色 hex | 用途 |
|---|---|---|---|---|
| 0 | **天蓝** | `#5AA9E6` | `#7FBFEE` | 默认/语文 |
| 1 | **蜜桃** | `#F38B6B` | `#FFA38A` | 数学 |
| 2 | **薄荷** | `#5DBB6A` | `#7CCE89` | 英语 |
| 3 | **芒果** | `#F0B73D` | `#FFC664` | 补习 |
| 4 | **葡萄** | `#9D7DD6` | `#B79DE8` | 兴趣 |
| 5 | **青瓷** | `#4DBFB3` | `#74D6CB` | 体育 |
| 6 | **嫩芽** | `#A6C84B` | `#BCDC72` | 科学 |
| 7 | **玫瑰** | `#E36B95` | `#F08FB0` | 美术 |

注：hex 是契约——代码用 `hex_color("#…")`（const fn，RGB→HSL 反算）生成 `COURSE_PALETTE` / `COURSE_PALETTE_DARK`（均为 `pub const`）。Phase 4 不需要看 HSL，只用 hex。

`tint(color, theme)`：`alpha = if dark { 0.22 } else { 0.16 }`，用于课程块的填充底（足够淡以不抢主色，足够浓以让色块有"课程味道"）。

### 2.7 图表色（dashboard 课时分布）

5 阶蓝（gpui-component 的 `chart_1..5` 已有 blue ramp）。我们覆盖为：

| Token | 亮色 | 暗色 | 用途 |
|---|---|---|---|
| `chart_1` | `#A9D5F2` | `#3D7CA8` | 备用最浅阶（当前图表未用） |
| `chart_2` | `#5AA9E6` | `#5AA9E6` | **dashboard 柱形唯一用色**：今日满色 / 其余 α0.45（§3.16） |
| `chart_3` | `#3F8FD0` | `#7FBFEE` | 备用中间阶（未用） |
| `chart_4` | `#2E73B5` | `#A8D2F2` | 备用中间阶（未用） |
| `chart_5` | `#1F5C97` | `#CFE5F8` | 备用最深阶（未用） |
| `chart_bullish` | `#3D8246` | `#7CCE89` | 上升 |
| `chart_bearish` | `#9C2929` | `#F08B8B` | 下降 |
| `chart_grid` | `#EADFC8` | `#3A342C` | 网格线 |

**柱形取色（与 §3.16 同一条规则，v1.4 对齐）**：本系统的手写柱形图只用 `chart_2`——今日柱 `theme.chart_2` 满色 + 数值 600，其余日 `theme.chart_2.with_alpha(0.45)`。`chart_1/3/4/5` 是留给未来图表（gpui-component chart）的 ramp，当前不进 dashboard。

### 2.8 图标尺寸

| Token (`ui::tokens`) | px | vendor `Size` | 用途 |
|---|---|---|---|
| `ICON_XS` | 12 | `Size::XSmall` | inline（chip 内、紧贴文字、小火焰） |
| `ICON_SM` | 14 | `Size::Small` | 默认（按钮内、tab、nav item） |
| `ICON_MD` | 18 | `Size::Size(px(18.))` | 卡片标题旁、状态徽标、stat_tile 右上角 |
| `ICON_LG` | 22 | `Size::Size(px(22.))` | 大号交互按钮 / 紧凑空态 |
| `ICON_XL` | 32 | `Size::Size(px(32.))` | 空态 Compact、32px 大图标场景 |
| — | 56 | `Size::Size(px(56.))` | 空态插画（Illustrated / Celebratory） |

**vendor 真实映射**（gpui-component 0.7.3 `icon.rs`）：`Size::XSmall / Small / Medium / Large`
= **12 / 14 / 16 / 24 px**（`size_3 / size_3p5 / size_4 / size_6`，rem=16）。
**没有 18/22/32**——这三种必须显式写 `Size::Size(px(n))`（stat_tile 18、empty_state 56/32 都是这样落的）。
`ICON_*` 是本系统的语义常量，供布局/自绘与视图层引用，不要假设它等于 vendor `Size`。
本系统控件内图标默认 `Small`（14）。

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
| `spring_control` | `Spring::new(220ms)` 阻尼 0.85 | 保留令牌（打卡弹跳 v1.4 起走 `bounce_animation`，见 §6.5） |
| `spring_move` | `Spring::new(320ms)` 阻尼 0.85 | 拖拽/重排 |

**reduce-motion 兜底**：所有用 `with_animation` 的组件自动遵守（`AnimationElement` 在 `cx.reduce_motion()` 时直接渲染终态）。**Phase 4 写视图时直接调用 `ui::motion::page_enter_animation()` / `ui::motion::bounce_animation(theme)`**（app.rs 用前者做页面转场，打卡勾选用后者，见 §3.17）。gpui-base 的 `spring(...)` 需要 `&mut Window`，视图 `render(app, cx)` 拿不到——因此 `motion` 模块不提供可播放的 spring（旧 `bounce_spring` 已删除）。

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
  - hover：bg `list_hover`，过渡 `140ms`。
  - active：bg `list_active`（透明橙），**图标 + 文字 `foreground`（深墨，11.0:1）**；
    左侧 3px 圆角竖条 `primary`（贴在 item 左缘内 8px）。强调色由竖条 + 底色承担，
    文字不再用 `primary`（`primary` 在 `list_active` 复合底上只有 1.83:1，不满足 AA）。
- **徽标**：在 `nav_item` 右侧，圆形 `rounded_full`，bg `danger`（过期提醒）或 `warning`（待打卡），文字 12px 自动选暖墨/纸白（≥4.5，见 §3.6）。空时不渲染。
- **Footer 同步指示**：一个小圆点 6px + 一行 12px 文字（"已同步 / 未登录 / …"，详细在 §3.4）。点击 → 跳到设置页。

#### 3.1.2 顶栏（Topbar）

- **Anatomy**：`title` 块（20px 加粗页面名 + 12px 灰色副标题/统计，下边自动隐藏）；右侧 `actions`：`sync_pill` + `theme_toggle` 图标按钮 + `quick_add` 主按钮。
- **背景**：`topbar`（与 panel 同）。`border_b_1px border_color=border`。高度 56px，左右 padding 24。
- **副标题**：根据当前页生成（如 dashboard："10月17日 周六 · 今日 2 节课 · 待打卡 2 项"；courses："已建 7 门课程 · 2 项已归档"）。由 `ui::components::page_header::page_subtitle(page, db)` 提供（**不在** `overlay.rs`）。
- **快速添加（`quick_add`）**：顶栏唯一的 primary 按钮，执行当前页的主力创建动作——课程管理→新建课程；计划表→新增安排；日程安排→新建日程；提醒事项→新建提醒；打卡任务→新建任务；备忘笔记→新建备忘；今日概览/设置→新建课程（现有兜底）。页面内容区不得再出现同义主按钮。
- **Motion**：页面切换时整个 content 区做 260ms 淡入（opacity 0→1 + translateY(4px → 0)），每页唯一动画 id（`page-enter:{label}`），尊重 reduce-motion（`ui::motion::page_enter_animation()`）。

#### 3.1.3 Content frame

- 页面 padding：**20**（`p_5`）——**所有宽度统一**，不存在按宽度的分档（v1.3 前的文档写过「≥1280 用 24 / 更窄用 20」，代码从未实现；V1/V2 泳道最初自己用了 `SPACE_2XL`（24），v1.4 已把 8 页统一为 `p_5`）。
  - 1280 下换算：内容区 = 1280 − 256（侧栏）− 2×20 = **984**；schedule 的 696 网格最小宽就是按「左右各 20、共 40」推导的（左栏 = 984 − 260 − 16 = 708 ≥ 696，见 §4.3）。
- 默认 `flex flex_col gap: 20`（`SPACE_XL`；settings 卡堆叠用 16）。
- 页面 body 以 `section_title` 开头，写**分节名**（如"本周安排"），不写页面名；dashboard 的问候语、settings 的卡片堆叠可直接作为首块，不强制 `section_title`。
- **不放** `page_header`、不放页面名、不放主 CTA（见 §3.1.4）。

#### 3.1.4 标题与 CTA 归属（一条规则）

**顶栏独占页面名 + 副标题 + 全局动作（`sync_pill` / `theme_toggle` / `quick_add`），`quick_add` 执行当前页的主力创建动作；页面内容区只用 `section_title` 分节，不得重复页面名，也不得再放同义的主（primary）CTA。**

- `page_header(theme, title, subtitle, actions)` 只保留给**多步骤/子视图**的英雄区（例如未来详情页）；`title` 不得等于 `Page::label()`，`actions` 只放次级操作。
- 页面特有的**次级动作**（如"全部标记已读"、筛选、导出）用 `ghost` / `icon_only` 放在相关区块的 `section_title` 右侧，不抢顶栏的主 CTA。
- Phase 4 三泳道实现任何页面时，先按本条对齐，再看 §4.x 的具体结构。

### 3.2 按钮

API：`ui::components::button::{primary, ghost, danger, icon_only, link}`。每种都接受 `(id, label, on_click, leading_icon?, trailing_icon?)`。

| 变体 | bg | fg | border | hover | active | 高度 | padding-x |
|---|---|---|---|---|---|---|---|
| `primary` | `primary` | `primary_foreground` | — | `primary_hover` | `primary_active` | 36 | 16 |
| `secondary` | `secondary` | `secondary_foreground` | 1px `border` | `secondary` + alpha +0.06 | alpha +0.10 | 36 | 16 |
| `ghost` | transparent | `foreground` | — | `list_hover` | `list_active` | 36 | 12 |
| `danger` | `danger` | `danger_foreground` | — | alpha +0.08 | alpha +0.14 | 36 | 16 |
| `icon_only` | transparent（ghost） | gpui-kit ghost 默认 | — | `list_hover` | `list_active` | 32×32 | 0 |
| `link` | transparent（link） | `link`（亮 `#2E6A99` / 暗 `#7FBFEE`） | — | fg `link_hover` + underline | fg `link_active` | 24 | 4 |

- **Icon-only** 默认 hover 背景 `list_hover`、active 背景 `primary.with_alpha(0.18)`，用于顶栏切换主题、关闭等。
- 文字走 gpui-kit Medium 尺寸（约 14/600）；图标 14px。
- **圆角**：`rounded_lg` (14px)；icon_only 为 10px。
- **Motion**：按下时 `transform: scale(0.97)` 持续 80ms；松手回弹。

### 3.3 Nav item（侧栏导航项）

API：`ui::components::nav_item::nav_item(theme, id, icon, label, active, badge: Option<(usize, Hsla)>, on_click)`。Anatomy：`[icon 14px] [label 14/600] [badge?]`。
- 默认高度 36px，左右 padding 12px；active 左侧 3px 圆角竖条 `primary`，图标与文字均为 `foreground`（深墨，11.0:1；`primary` 文字在 `list_active` 上只有 1.83:1）。
- Badge：min 18×18 圆；`badge` 的第二项是**语义色**（过期 `danger`、待打卡 `warning`），与 active 无关；文字 11/700 自动选暖墨/纸白（不足时纯黑/纯白）保证 ≥4.5:1。
- 点击整行触发 on_click（不是图标）。
- test-support id：`eid("nav", p.label())`（保持与 Phase 2 一致，QA 可继续使用）。

### 3.4 Sync 状态 pill

API：`ui::components::sync_pill::sync_pill(theme, id, status: &Status, on_click)`；侧栏用 `sync_indicator_compact(theme, id, status, on_click)`。Anatomy：圆角胶囊 `rounded_full`，高 24px，padding-x 10px，左侧 6px 圆点（颜色随状态），右侧 12px 文字。

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
- `ui::components::tag::tag(theme, text, accent: Hsla)`：圆角胶囊 22px 高，padding-x 10，bg `accent.with_alpha(0.18)`，文字 12/600；文字色优先 accent，若在 tint 上 <4.5 则沿同色相加深/提亮，再退暖墨/纸白，最后以纯黑/纯白兜底——保证任意 tint ≥4.5。
- `ui::components::badge(count: usize, accent: Hsla)`：圆形 18×18，文字 11/700；文字色自动在暖墨/纸白里取对比度更高者，二者都不足时用纯黑/纯白（对任意 fill ≥4.5，含 `#777777` 这类中灰）。
- `ui::components::chip(theme, text, accent: Hsla)`：与 tag 视觉相似但更小（20px 高）。
- `ui::components::chip_with_filled(text, color, filled)`：旧签名兼容层（views 在用）；filled=true 实心，文字同样自适应。

差异：tag 用在卡片标题旁表示分类（"补习"/"校外"），chip 用在行内描述后（"每天 · 累计 0 次"），badge 用在导航/计数。

### 3.7 卡片（Card variants）

API：`ui::components::card::card_with_theme(theme, variant, title?, subtitle?, content)`，variant ∈ {`Default`, `Flat`, `Outlined`, `Accent(color)`, `Hero`}。**没有**不传 theme 的同名 `card`（颜色无法正确解析）。

| 变体 | bg | border | 圆角 | shadow | 用途 |
|---|---|---|---|---|---|
| `Default` | `panel` | 1px `border` | `lg` (14) | none | 标准卡片 |
| `Flat` | `transparent` | — | — | none | 嵌入父容器（如 dashboard 时间轴） |
| `Outlined` | `surface` | **2px** `border` | `lg` | none | 强调分组 |
| `Accent(c)` | `c.with_alpha(0.10)` | 1px `c.with_alpha(0.4)` | `lg` | none | 课程块/统计高亮 |
| `Hero` | `panel` | 1px `border` | `xl` (20) | **`shadow_md`** | dashboard 顶部 4 个统计卡 |

- 标题 17/600 `foreground`，副标题 14/400 `muted_foreground`；内边距 16，标题区与内容间距 12。
- **hover**：`Default` 卡片 hover 时 `border_color: border_strong`（`ThemeColor.input`），不变 shadow；组件已实现（`card.rs` 的 `.hover`）。`Flat`（无边框）与 `Accent`（色调边框）不做 hover 边框。
- `Hero` 用于 dashboard 4 个统计卡，有 shadow 表达"重要数据"。

### 3.8 Stat tile

API：`ui::components::stat_tile::stat_tile(theme, label, value, hint?, accent: Hsla, icon: Option<IconName>)`。用于 dashboard 顶部 4 块。
- 高度 `min_h: 108`，padding 16。
- 顶部：左侧 label 12 `muted_foreground`，右侧 icon `accent` 18px。
- 中部：value 32/700 `foreground`。
- 底部：hint 12 `muted_foreground`。
- 圆角 `rounded_xl` (20)，bg `panel`，1px `border`。

### 3.9 List row

API：`ui::components::list_row::list_row(theme, id, on_click?, leading?, title, subtitle?, trailing?)`。
- 高度 auto，最小 48，gap 12，padding-x 12。
- 默认 bg `transparent`（不叠第二层纸色），hover bg `list_hover`，active bg `list_active`（按下），圆角 `rounded_md`。
- 用于：dashboard 今日课程时间块、课程管理/日程/提醒/备忘的列表项。

### 3.10 Progress / Streak

API：`ui::components::progress::progress(theme, value: f32, accent: Hsla, height: Pixels /* 6 or 10 */)`。
- 圆角 `rounded_full`，bg `muted` 容器，`accent` 填充；用于：checkin 进度条、staple 卡片底部条。

API：`ui::components::progress::streak_badge(theme, days: usize)`：火焰图标 + "连续 N 天"，文字 12/700 `warning`（亮色 `warning` 在 panel 上 4.83:1）。

### 3.11 Empty state

API：`ui::components::empty_state::empty_state(theme, variant, icon, title, description?, action?)`。variants:
- `Illustrated`：用于整页空态（dashboard/memos/reminders）。容器 320×240，居中，图标 56px `muted_foreground`，标题 20/600 `foreground`，描述 14 `muted_foreground`，底部 action 按钮。
- `Compact`：用于列表内空态。容器 padding-y 24，icon 32px，标题 15/600，描述 12。
- `Celebratory`：用于"全部完成"（checkin 全部 done）。**固定 `IconName::Sparkle` 56px `warning`**，右上角再叠一颗 20px `Sparkle`，标题 22/700。容器**不套** Illustrated 的 320×240 大框——随内容收紧（padding 20、圆角 20，在浮层里居中），否则庆祝卡会大片留白。
- 文案示例见 §1.3。
- Phase 4 不得自定义空态结构，必须用此组件。

### 3.12 Overlay / Dialog / Backdrop

API：
- `ui::components::overlay::overlay(theme, content)`：全屏绝对定位 backdrop + 居中容器。backdrop 直接用 `theme.overlay`（亮色 `foreground` α0.32 / 暗色 α0.55，`ThemeConfig` 以 `#RRGGBBAA` 保存）。容器 `bg: panel`、`rounded_2xl` (28)、大阴影、border、padding 24、min-width 420、max-width 560。
- **点击拦截内建（v1.4）**：`overlay` 根节点带 `occlude()`（`HitboxBehavior::BlockMouse`）。GPUI hit-test 反向收集 hitbox——面板子元素先入列、走到遮罩根即停止，所以面板仍可交互、而遮罩**下面**的页面行 / 卡片收不到鼠标事件。视图层不得再自建「透明 guard 层」（旧的 `overlay_click_guard()` 已删除）。
- `dialog(theme, title, body, footer)`：基于 `overlay` 内容的列布局，顶部 title 17/600，底部按钮行右对齐。
- `alert_dialog(theme, id_prefix, title, description, ok_label, cancel_label, ok_variant, on_ok, on_cancel)`：基于 `dialog`；`id_prefix` 生成唯一 id（`{prefix}:ok` / `{prefix}:cancel`），cancel 必须接真实关闭回调。
- `first_login_prompt(theme, probe, on_choice)`：**专属三选一首登**——结构详见 §3.13。

**已接受限制**：`overlay` 挂在页面根（`absolute().inset_0()` 于 content 区），所以模态只压暗**内容区**，侧栏 / 顶栏仍可交互。要整窗压暗需要一个 shell 级 dialog host——推迟到后续版本（§7）。

### 3.13 First-login dialog（首登三选一）

保留现有 `App::first_login_prompt` 逻辑；渲染已统一走 §3.12 组件。视觉规格：
- 用 §3.12 `dialog` 包装；title "首次登录：云端与本地都有数据"；容器宽 480。
- body 上半部分：列出云端各实体数量（"课程 × 3、课表 × 17、日程 × 2"），用 accent tint 卡片列出。
- body 下半部分：一段 12px 警示文字（"「本地覆盖云端」会为云端独有数据推送删除（墓碑）。"）。
- footer：合并（推荐）= primary；云端覆盖本地 / 本地覆盖云端 = secondary；右侧 24px 高 link 按钮"稍后决定"（`FirstLoginAction::Cancel`，只关窗不发策略）。
- 稳定 id：`first-login:ok` / `first-login:cloud` / `first-login:local` / `first-login:cancel`。

### 3.14 Form field row

API：`ui::form::field_row(label, control)`（已存在，签名不变）。label `caption` `muted_foreground`，固定宽 84px，右边控制填满。

**对话框表单的唯一标准（v1.4 统一）**：所有编辑器对话框（课程 / 安排 / 日程 / 打卡 / 提醒 / 备忘）的字段一律用 `field_row`（label 左置）——包括输入框、`Select`、`DatePicker` / `TimeField`、`tab_chips`、备注 `Textarea`、色板与星期选择行。禁止「label 在控件上方」的写法：
- 自定义 `ui::select::Select::label()` 会把标签渲染在触发器的**上方**——对话框里不要传 `.label()`，改由 `field_row` 给 label（该方法仅保留给对话框之外的内联场景，当前无调用点）。
- 备注 Textarea 也要包 `field_row("备注", ...)`（V3 提醒编辑器是最早的样板）。

### 3.15 Input / Select / Date / Time

Phase 4 必须使用 gpui-kit 原生组件，**禁止**继续用 `<input type="date">` 文本：
- `Input` → `InputState` + `Input`。
- 日期 → `DatePicker`（gpui-component 自带，自带 Calendar 下拉）。
- 时间 → `TimeField`。
- 单选/多选 → `Select`（gpui-component）或现有 `ui::select::Select`（保留用于自定义内联展开）。

**统一外观**：所有输入控件边框 `border_strong`、hover `primary.with_alpha(0.5)`、focus `primary` + 2px ring `ring`；圆角 `rounded_lg`；高度 36；padding-x 12；font `body`。

### 3.16 Chart

dashboard "本周课时分布" 当前手写柱形图。Phase 4 V1 **可保留手写版本**（信息密度好），也可替换为 `chart::bar`（gpui-component）。**若保留**（现状即保留）：只用 `theme.chart_2` 一种色——非今日柱 `chart_2.with_alpha(0.45)`，今日柱 `chart_2` 满色 + 数值 600 加粗；圆角 `rounded_md` (10)。与 §2.7 的说明同源。

### 3.17 Check-in celebration（打卡庆祝）

打卡勾选触发。Spec：
- 触发：`on_click` → 设置 done → 立刻在打卡项上 spring 一下：`scale(1 → 1.06 → 1)` 280ms ease_out_back。
- 同时勾选圆环 `success` 实心填充从 0% → 100% 在 200ms 内。
- **若当日所有打卡任务完成**：触发"庆祝瞬间"——中心 `Sparkle` icon 浮现 + scale(0 → 1) 320ms + rotate(0 → 30deg) 400ms；同时背景从 `surface` → `success.with_alpha(0.08)` 在 600ms 内淡入。1200ms 后回淡至 surface。
- **reduce-motion**：跳过所有 scale/rotate，只保留颜色变化。

**实现注（v1.4）**：本 gpui 分支没有 CSS transform（`scale()`/`rotate()`），弹跳以「圆环直径 + 短促光晕」表达（`checkins::check_burst`）；动画函数是 `ui::motion::bounce_animation(theme)` + `bounce_pulse(delta)`（§6.5），reduce-motion 由 `with_animation` 自动落到终态。

### 3.18 Page transition（页面切换淡入）

content 区进入动画：opacity 0 → 1 + translateY(4px → 0)，`duration_slow` (260ms) `easing_enter`。由 `Presence` + `transition` 实现，监听 `App.page` 变化。reduce-motion 时立即显示。

---

## 4. 8 个页面的布局规格

每个页面给出：**结构**、**层级**、**关键交互**、**Phase 4 必须保留的信息密度与行为**。Phase 4 写视图时按此规范结构 + §3 组件库落地。

> **统一规则（§3.1.4）**：页面名 + 副标题 + 全局动作只在顶栏，`quick_add` 执行该页主力创建动作；内容区不重复页面名、不放第二个同义主（primary）CTA，只用 `section_title` 分节，页面特有操作以次级按钮就地出现。以下各页不再重复这条。

### 4.1 今日概览（Dashboard）——V1

- **结构**（自上而下）：
  1. Greeting 区：`display` 字号 "早上好，今天也要加油！"（问候语，不是页面名）；副标题 `body_sm` "10月17日 周六 · 今日 2 节课 · 待打卡 2 项"。
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

- **结构**：顶栏显示「课程管理」+ 副标题，`quick_add` = 新建课程。内容区以 `section_title`（如"全部课程"）开头，右侧只放次级动作；下方 `tab_chips`（全部 / 校内 / 补习）→ 课程列表。**没有**页面 `page_header`，也没有第二个"新建课程"主按钮。
- **内容**：课程卡片列表，每行：
  - 左侧 4px 圆角色条（按 `Course.color`）。
  - 中间：标题 + kind chip（"校内"或"补习"，info/warning）+ 状态 chip（已归档）；副标题：科目 + 老师 + 地点；底部：days summary + weekly 分钟数；备注（如有）。
  - 右侧：编辑 / 删除 action_button。
- **关键交互**：tab_chips 切换过滤；点击卡片中间区域 → 编辑器（Phase 4 V2 改为编辑模式 inline 或 modal）；编辑 / 删除按钮各自 on_click。
- **保留行为**：过滤（全部/校内/补习）、8 色调色板（按 `Course.color` 索引取）、已归档显示状态 chip、删除二次确认。

### 4.3 计划表（Schedule）——V2

- **结构**：顶栏显示「计划表」+ 副标题（如"按周重复 · 本周 12 条"），`quick_add` = 新增安排。内容区以 `section_title`（如"本周安排"）开头，右侧只放次级动作（如"今天"、筛选）；**没有**页面标题与"新增安排"主按钮。下方两栏：
  - **左**：7 列周表头（周一/周二/…/周日）+ 卡片格；今天是 `today` 列高亮（border 1.5px primary + bg `primary.with_alpha(0.04)`）。网格最小宽度 **696** = 56（时间槽列）+ 7×88（日列）+ 6×4（列间距），容器更窄时整体水平滚动。
  - **右**（260px）：本周概况 card（4 个 `summary_row` 用 §3.6 tag 颜色块显示校内/补习/合计/条数）+ 安排清单 card（12 条 list_row，按 weekday+start 排序）。
- **关键交互**：点击表头 → 切换以那天为中心？**否**：保持现状，点击空白格 → 新增该 weekday；点击现有块 → 编辑；hover 整列 → 列底 `+` 按钮变 `primary`，点击新增该日。
- **保留行为**：7 列布局完整；当天列高亮；色块按课程色；右侧概况 + 安排清单；删除二次确认。

### 4.4 日程安排（Events）——V2

- **结构**：顶栏显示「日程安排」+ 副标题，`quick_add` = 新建日程。内容区：
  - 顶部：日期选择器（左右箭头 + 当前日期 + "今天"快捷按钮）。
  - `section_title`（如"当天安排"）。
  - 当日日程 list：每行 8px 圆点（按关联课程色或 primary）+ 时间段 + 标题 + 地点 + 完成 checkbox。
  - 右侧 220px「当天课程」side rail（`RAIL_W`）：按当前选中日期取 `sessions_on(weekday)`，每行课程色圆点 + 课程名 + 时间段（`event-course:{session_id}`）。**v1.4 确认有意保留**（Phase 2 的既有行为与信息密度，V2 未删）。日程清单为空时 rail 照常显示"这一天没有课程"。
- **关键交互**：日期切换；点击行 → 编辑；勾选完成 → 划线 + 灰色。
- **保留行为**：日期导航；按日期筛选；已完成状态视觉；与课程关联；右侧当天课程 rail。

### 4.5 提醒事项（Reminders）——V3

- **结构**：顶栏显示「提醒事项」+ 副标题，`quick_add` = 新建提醒。内容区：
  - 顶部：`tab_chips`（待办 / 已完成，可切换 `show_done_reminders`）。
  - 提醒列表（`section_title` 如"待办提醒"）：每行 8px 圆点（`Once` 提醒 = warning，循环提醒 = info）+ 标题 + 内容预览 + due_at + repeat label + 编辑 / 删除。
  - 底部空白区可放已完成列表（当 `show_done=true`）。
- **关键交互**：切换待办/已完成；勾选完成；编辑；删除。
- **保留行为**：到期高亮（due 在过去但未完成 → row 左侧加 warning 边）；重复类型标签；本地通知（`start_timer` 不动）。

### 4.6 打卡任务（Check-Ins）——V1

- **结构**：顶栏显示「打卡任务」+ 副标题，`quick_add` = 新建任务。内容区：
  - `section_title`（如"今日打卡"）。
  - 顶部：今日完成率 progress 条（0% 文字 + 条 + N/M 数字）。
  - 3 列看板：`待打卡`（今日 active 未 done）/ `今日已完成`（今日 active done）/ `全部任务`（未归档）。每列：
    - 列头：标题 + 计数。
    - 任务卡片：check_circle + 标题 + 描述 + repeat chip（"每天"等）+ weekday chip + streak + 编辑/归档 action_button。
- **关键交互**：勾选打卡 → 触发庆祝动效；编辑；归档（移到归档列表，下次不再出现）。**归档需二次确认**（AlertDialog）。
- **保留行为**：3 列看板；streak 计算；今日 active 逻辑；repeat/weekday 表达；归档二次确认。

### 4.7 备忘笔记（Memos）——V3

- **结构**：顶栏显示「备忘笔记」+ 副标题，`quick_add` = 新建备忘。内容区：
  - `section_title`（如"全部备忘"）。
  - `tab_chips`（全部 / 已置顶）。
  - 3 列网格（`grid_cols(3)`）：每张备忘卡：左侧 4px 角色条（按 `Memo.color`）+ 标题 + 内容（多行省略）+ 更新时间 + 置顶 chip（pinned=true 时）+ 编辑/置顶/删除。
- **关键交互**：切换 tab；点击编辑；切换置顶；删除。
- **保留行为**：置顶置顶；按 pinned + updated_at 排序；删除二次确认。

### 4.8 设置（Settings）——V3

- **结构**：顶栏显示「设置」+ 副标题；本页没有创建动作（`quick_add` 走 §3.1.2 的兜底 = 新建课程）。内容区为单列卡片堆叠（间距 16），首块直接是"学生信息"卡片，无页面标题：
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

所有数字由 `theme::contrast_ratio`（WCAG 2.1 相对亮度）与 `ui::components::tag` 的自适应选色共同保证，测试见 `theme::tests::wcag_aa_contrast_on_filled_buttons`、`theme::tests::wcag_aa_shell_nav_and_link_pairs`、`tag::tests::*`。

| 前景 / 背景 | 亮色 | 暗色 | 阈值 |
|---|---|---|---|
| `foreground` on `surface` | **13.96:1** | **14.65:1** | ≥7 |
| `foreground` on `panel` | **14.68:1** | **13.08:1** | ≥7 |
| `muted_foreground` on `surface` | **5.14:1** | **6.40:1** | ≥4.5 |
| `muted_foreground` on `panel` | **5.40:1** | **5.72:1** | ≥4.5 |
| `muted_foreground` on `sidebar`（inactive nav） | **4.68:1** | **6.20:1** | ≥4.5 |
| `muted_foreground` on `muted`（sync pill 离线） | **4.54:1** | **4.93:1** | ≥4.5 |
| active nav `foreground` on `list_active`（叠 sidebar） | **11.00:1** | **12.94:1** | ≥4.5 |
| `link` on `panel` | **5.63:1** | **7.96:1** | ≥4.5 |
| `link` on `surface` | **5.36:1** | **8.91:1** | ≥4.5 |
| `link_hover` / `link_active` on `panel` | 6.61 / 7.90 | 9.14 / 6.40 | ≥4.5 |
| `primary_foreground` on `primary` | **7.10:1**（深棕 ink） | **9.08:1**（深棕） | ≥4.5 |
| `primary_foreground` on `primary_hover` | 5.96:1 | — | ≥4.5 |
| `primary_foreground` on `primary_active` | 4.57:1 | — | ≥4.5 |
| `accent_foreground` on `accent` | **4.74:1**（白） | **8.43:1**（深蓝） | ≥4.5 |
| `success_foreground` on `success` | **4.69:1**（白） | **8.64:1** | ≥4.5 |
| `warning_foreground` on `warning` | **4.95:1**（白） | **8.80:1** | ≥4.5 |
| `danger_foreground` on `danger` | **7.60:1**（白） | **7.36:1** | ≥4.5 |
| `secondary_foreground` on `secondary` | **8.26:1** | **10.33:1** | ≥4.5 |
| badge text（自动选白/墨）on `danger` | **7.60:1** | **7.69:1** | ≥4.5 |
| badge text（自动选白/墨）on `warning` | **4.95:1** | **11.86:1** | ≥4.5 |
| `streak_badge` warning on panel | 4.83:1 | 10.16:1 | ≥4.5 |

**策略**：亮色对饱和色使用加深档 fill + 白字；主橙保留亮色但文字用深棕 ink。暗色使用提亮 fill + 深色字。链接单独用 `link` 家族（亮色 `#2E6A99` 系、暗色 `#7FBFEE` 系），**不用 `primary`**。`tag` / `filled chip` / `badge` 不写死白字——按背景亮度自动在暖墨/纸白间选择；半透明 tint 先尝试 accent 本身、不足则同色相调整；暖墨/纸白都不足 4.5 时（如 `#777777` 中灰，两者仅 4.20/4.41）由纯黑/纯白兜底（该兜底对任意背景 ≥4.58），整体保证 ≥4.5。历史上"浅色芒果黄 + 白字 1.9:1"的问题已由 `text_on_fill` 修复。

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
- 由 `theme.motion` 自动遵守；本 gpui 分支没有可播放的自定义 spring（需要 `&mut Window`），弹跳统一走 `ui::motion::bounce_animation`（§6.5），`with_animation` 在 reduce-motion 时直接渲染终态。

---

## 6. 组件 API Freeze（Phase 4 必须遵守）

Phase 4 三条 lane 只能使用以下公开 API。**签名与命名是契约**——本节的每一行都从
`src/` 真实签名抄录；任何偏离需要回到本文件更新签名再改代码。

### 6.1 `src/theme.rs`

```rust
pub fn init(cx: &mut App)                      // 幂等；安装 light/dark ThemeConfig + apply
pub fn apply_for_mode(cx: &mut App)            // 直接写 colors/tokens/radius/font/motion
pub fn toggle_mode(window: Option<&mut Window>, cx: &mut App)
```

`init` 把 CoursePlan `ThemeConfig` 装入 `theme.light_theme` / `theme.dark_theme`，因此直接
`Theme::change(mode)` 也不会回退到 gpui-component 默认色。`theme_config_colors_from` 的
键集合 / alpha 由 `theme::tests::theme_config_keys_match_vendor_schema` 与
`direct_theme_change_preserves_all_semantic_fields` 锁定。

**已接受限制（12 个私有 base 色）**：`ThemeConfigColors` 的 12 个私有字段
（`base.red … base.cyan.light`）无法从 crate 外赋值，所以**直接** `Theme::change(mode)`
（不经过 `apply_for_mode`）之后，`theme.colors.red / red_light / green / green_light /
blue / blue_light / yellow / yellow_light / magenta / magenta_light / cyan / cyan_light`
会回退到 gpui-component 默认值。当前所有组件都读语义 token（`danger` / `success` /
`warning` / `info` / `magenta` 等），**不读这 12 个字段**，因此视觉不受影响；
`apply_for_mode` / `theme::init` / `toggle_mode` 仍会把它们写对。若未来组件改用
`theme.red` 等基础色，必须在 `Theme::change` 后补一次 `apply_for_mode`。

### 6.2 `src/ui/mod.rs`

```rust
pub mod tokens;
pub mod palette;
pub mod motion;
pub mod components;
pub mod form;
pub mod select;
pub mod widgets;  // 兼容层（Phase 4 逐步删除）
```

### 6.3 `src/ui/tokens.rs`

```rust
pub const SPACE_XS/XS...: Pixels;   // SPACE_XS=4, SM=8, MD=12, LG=16, XL=20, 2XL=24, 3XL=32
pub const RADIUS_SM/MD/LG/XL/2XL/FULL: Pixels; // 6/10/14/20/28/9999
pub const BUTTON_H: Pixels = px(36.);
pub const ICON_BUTTON_H: Pixels = px(32.);
pub const INPUT_H: Pixels = px(36.);
pub const NAV_ITEM_H: Pixels = px(36.);
pub const SYNC_PILL_H: Pixels = px(24.);
pub const SIDEBAR_BRAND_H: Pixels = px(72.);
pub const SIDEBAR_FOOTER_H: Pixels = px(56.);
pub const TOPBAR_H: Pixels = px(56.);
pub const SIDEBAR_W: Pixels = px(256.);
pub const ICON_XS/SM/MD/LG/XL: Pixels; // 12/14/18/22/32
pub const COURSE_TINT_ALPHA_LIGHT: f32 = 0.16;
pub const COURSE_TINT_ALPHA_DARK: f32 = 0.22;
pub const SYNC_PULSE_MS: u64 = 1200;

pub mod text {
    pub const CAPTION/LABEL_SM/BODY_SM/BODY/BODY_EMPH/LABEL/SUBTITLE/TITLE/DISPLAY/STAT: Pixels;
    // 12/13/14/15/15/14/17/20/28/32
}
```

**没有 `tokens::color`**：颜色是主题态，必须从 `&Theme` 读取（`theme.background`、`theme.primary`…），
禁止在 token 层固化颜色。

### 6.4 `src/ui/palette.rs`

```rust
pub const COURSE_PALETTE: [Hsla; 8];       // 亮色盘
pub const COURSE_PALETTE_DARK: [Hsla; 8];  // 暗色盘
pub fn palette_at(index: usize) -> Hsla;                 // 无主题兼容；新代码别用
pub fn palette_at_for(index: usize, theme: &Theme) -> Hsla; // Phase 4 必须用这个
pub fn tint(color: Hsla, theme: &Theme) -> Hsla;
```

### 6.5 `src/ui/motion.rs`

```rust
pub fn page_enter_easing() -> Easing;
pub fn page_exit_easing() -> Easing;
pub fn move_easing() -> Easing;
pub fn page_enter_duration(reduce_motion: bool) -> Duration;
pub fn page_enter_animation() -> Animation;            // 260ms + easing_enter，with_animation 用
pub fn bounce_animation(theme: &Theme) -> Animation;   // 打卡弹跳：duration_slow + ease-out-back 过冲
pub fn bounce_pulse(delta: f32) -> f32;                // 折成「0 → 1 → 0」脉冲（终态恒 0）
pub fn tokens(theme: &Theme) -> &MotionTokens;
```

**不存在** `pop_in` / `pop_out` / `spring_bounce`：Phase 3 冻结时它们未实现，已从 API 移除。
页面转场已由 `app.rs` 用 `with_animation` + `page_enter_animation()` 落地。

**`bounce_spring` 已删除（v1.4）**：它返回 gpui-base 的 `Spring` 策略，只能交给
`gpui_base::motion::spring(id, target, policy, window, cx)`——需要 `&mut Window`，而视图的
`render(app, cx)` 拿不到；本 gpui 分支也没有 CSS transform。打卡弹跳统一走
`bounce_animation(theme)` + `bounce_pulse(delta)`，由调用方把脉冲折进几何 / 颜色
（`views::checkins::check_burst`：圆环直径 + 光晕）。reduce-motion 由 `with_animation` 自动落到终态。

### 6.6 `src/ui/components/` 模块

全部组件第一个参数是 `theme: &Theme`（按钮族除外——按钮颜色由已安装的 ThemeConfig
经 gpui-kit ButtonVariants 解析，签名保持无 theme）。

#### `button.rs`

```rust
pub fn primary(id, label, on_click) -> Button;
pub fn primary_with_icon(id, label, icon: IconName, on_click) -> Button;
pub fn secondary(id, label, on_click) -> Button;
pub fn secondary_with_icon(id, label, icon, on_click) -> Button;
pub fn ghost(id, label, on_click) -> Button;
pub fn ghost_with_icon(id, label, icon, on_click) -> Button;
pub fn danger(id, label, on_click) -> Button;
pub fn danger_with_icon(id, label, icon, on_click) -> Button;
pub fn icon_only(id, icon, tooltip, on_click) -> Button;
pub fn link(id, label, on_click) -> Button;
```

统一尺寸：文字按钮 36px 高 / padding-x 16（ghost 12）/ 圆角 14；icon_only 32×32 / 圆角 10；
link 24px / padding-x 4。

#### `nav_item.rs`

```rust
pub fn nav_item(
    theme: &Theme,
    id: ElementId,
    icon: IconName,
    label: impl Into<SharedString>,
    active: bool,
    badge: Option<(usize, Hsla)>,   // (count, 语义色)
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement;
```

#### `sync_pill.rs`

```rust
pub fn sync_pill(theme: &Theme, id: impl Into<ElementId>, status: &Status,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> impl IntoElement;
pub fn sync_indicator_compact(theme: &Theme, id: impl Into<ElementId>, status: &Status,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static) -> impl IntoElement;
```

#### `tab_chips.rs`

```rust
pub fn tab_chips<T, F>(
    theme: &Theme,
    id_prefix: &str,
    items: &[(&str, T)],
    selected: T,
    on_change: F,
) -> impl IntoElement
where T: Copy + PartialEq + 'static,
      F: Fn(T, &mut Window, &mut App) + Clone + 'static;
```

#### `tag.rs`

```rust
pub fn tag(theme: &Theme, text: impl Into<SharedString>, accent: Hsla) -> impl IntoElement;
pub fn badge(count: usize, accent: Hsla) -> impl IntoElement;
pub fn chip(theme: &Theme, text: impl Into<SharedString>, accent: Hsla) -> impl IntoElement;
pub fn chip_with_filled(text, color: Hsla, filled: bool) -> impl IntoElement; // back-compat
```

#### `card.rs`

```rust
pub enum CardVariant { Default, Flat, Outlined, Accent(Hsla), Hero }
pub fn card_with_theme(
    theme: &Theme,
    variant: CardVariant,
    title: Option<impl Into<SharedString>>,
    subtitle: Option<impl Into<SharedString>>,
    content: impl IntoElement,
) -> impl IntoElement;
```

**没有** `card(variant, title, subtitle, content)`（不带 theme 的同名函数已删除）。

#### `stat_tile.rs`

```rust
pub fn stat_tile(theme: &Theme, label, value, hint: Option<impl Into<SharedString>>,
    accent: Hsla, icon: Option<IconName>) -> impl IntoElement;
```

#### `list_row.rs`

```rust
pub fn list_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    on_click: Option<impl Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
    leading: Option<impl IntoElement>,
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    trailing: Option<impl IntoElement>,
) -> impl IntoElement;
```

#### `progress.rs`

```rust
pub fn progress(theme: &Theme, value: f32, accent: Hsla, height: Pixels) -> impl IntoElement;
pub fn streak_badge(theme: &Theme, days: usize) -> impl IntoElement;
```

#### `empty_state.rs`

```rust
pub enum EmptyVariant { Illustrated, Compact, Celebratory }
pub fn empty_state(theme: &Theme, variant: EmptyVariant, icon: IconName,
    title: impl Into<SharedString>, description: Option<impl Into<SharedString>>,
    action: Option<impl IntoElement>) -> impl IntoElement;
```

兼容 shim 在 `ui::widgets::empty_state(theme, icon, title, description)`，内部固定 Compact。

#### `overlay.rs`

```rust
pub fn overlay(theme: &Theme, content: impl IntoElement) -> impl IntoElement;
pub fn dialog(theme: &Theme, title: impl Into<SharedString>, body: impl IntoElement,
    footer: Option<impl IntoElement>) -> impl IntoElement;
pub fn alert_dialog(
    theme: &Theme, id_prefix: &str, title, description, ok_label, cancel_label,
    ok_variant: AlertVariant,
    on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement;
pub fn first_login_prompt(theme: &Theme, probe: &ProbeSummary,
    on_choice: impl Fn(FirstLoginAction, &mut Window, &mut App) + Clone + 'static) -> impl IntoElement;
pub enum FirstLoginAction { Merge, CloudWins, LocalWins, Cancel }
pub fn section_title(theme: &Theme, text, trailing: Option<AnyElement>) -> impl IntoElement;
```

#### `kv_row.rs` / `check_circle.rs`

```rust
pub fn kv_row(theme: &Theme, key, value) -> impl IntoElement;
pub fn check_circle(theme: &Theme, done: bool, size: Pixels) -> impl IntoElement;
```

#### `page_header.rs`

```rust
pub fn page_header(theme: &Theme, title, subtitle: Option<impl Into<SharedString>>,
    actions: Option<impl IntoElement>) -> impl IntoElement;
pub fn page_subtitle(page: Page, db: &Database) -> SharedString;  // 在此文件，不在 overlay.rs
```

### 6.7 `src/ui/widgets.rs`（Back-compat 兼容层）

```rust
pub const PALETTE: [Hsla; 8];
pub fn palette_at(index) -> Hsla;
pub fn surface(theme) -> Hsla;
pub fn panel(theme) -> Hsla;
pub fn tint(color, theme) -> Hsla;
pub fn eid(prefix: &str, key: &str) -> ElementId;   // "{prefix}:{key}"（无 "ui:" 前缀）
pub fn close_editor(app, cx);
pub fn action_button(id, label, on_click);
pub fn section_title(theme, text, trailing);
pub fn card(theme, title, subtitle: Option<..>, content);
pub fn stat_tile(theme, label, value, hint, accent, icon);
pub fn empty_state(theme, icon, title, description);
pub fn chip(text, color: Hsla, filled: bool);
pub fn kv_row(theme, key, value);
pub fn check_circle(theme, done, size);
pub struct Skin { surface, panel, sidebar, border, fg, muted, primary, primary_fg }
```

### 6.8 测试支持 IDs（QA/Phase 4 用）

shell 元素使用以下 id（**必须稳定**；`eid` 的结果不含 `ui:` 前缀，所以这里写出最终字符串）：

```
sidebar items:     eid("nav", page_label)          → "nav:今日概览" 等
brand block:       "ui:brand:"                     // 整块
theme toggle:      "ui:topbar:theme-toggle"
quick add:         "ui:topbar:quick-add"
sync pill:         "ui:topbar:sync-pill"
sidebar sync:      "ui:sidebar:sync-indicator"     // 可点击 → 设置页
first-login merge: "first-login:ok"
first-login cloud: "first-login:cloud"
first-login local: "first-login:local"
first-login cancel:"first-login:cancel"
```

**重复 ElementId 规则（V1 发现，v1.4 入册）**：同一实体在页面上出现多次时，id 必须带**分区前缀**，
否则 GPUI 只认第一个、点击/查询都会打错目标。打卡页同一任务同时在「待打卡 / 今日已完成 / 全部任务」
三列，因此是：

```
checkin toggle:    eid("checkin-toggle-pending"|"-done"|"-all", id)   → "checkin-toggle-pending:{id}"
checkin edit:      eid("checkin-edit-{pending|done|all}", id)
checkin archive:   eid("checkin-arch-{pending|done|all}", id)
checkin burst 动画: "checkin-burst:{col}:{id}:{done}:{gen}"             // {gen} 为点击世代号：点击才重放，首次挂载不挂动画
```

同理，`tab_chips` / 过滤器生成的 id 是 `{prefix}:{label}`（中文 label 原样拼进去）：

```
course filter:     "course-filter:全部" / "course-filter:校内" / "course-filter:补习"
reminder tabs:     "reminder-tab:待办" / "reminder-tab:已完成"
memo tabs:         "memo-tab:全部" / "memo-tab:已置顶"
```

页面自行加的新 id 也必须遵守这两条（唯一性 + 中文原样），否则 QA 的 `window.click(...)` 会命中错误元素。

### 6.9 图标注册（写入 `src/assets.rs`）

Phase 3 已登记：`CalendarDays / Clock / Flame / GraduationCap / ListTodo / NotebookPen /
Sparkle / Target / Trash`（`Sparkle` 为单数——gpui-kit 的真实 `IconName` 变体；
DESIGN 早期写的 `Sparkles` 不存在）。任何新增 `IconName::X` 必须先加入
`APP_ICONS` 与 `icon_assets!`；`tests/icons.rs` 会扫描源码兜底。

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

**已接受的限制**（Phase 4 收官（v1.4）确认，不阻塞发布）：
- **模态只压暗内容区**：`overlay` 挂在页面根（`absolute().inset_0()` 于 content 区，含删除确认与编辑器），侧栏 / 顶栏在模态打开时仍可点击。整窗压暗需要一个 shell 级 dialog host——推迟。
- **删除确认请求状态用 GPUI `Global`**：`PendingDelete` 全局存 `{page, kind, id, name}`；`App::goto` 在每次切页时清空它以及打卡页的归档确认状态（不再需要在各页 `discard_stale_delete`），因此切页返回不会重现旧对话框。`app.rs` 暂无对应字段槽位，保持现状。

---

## 8. 验证清单（Gate 3 证据）

- [x] `target/visual/*.png` 8 页 × 浅/深 = 16 张 + 3 张交互 smoke（base / editor / after-close）全部刷新。
- [x] `cargo test --workspace` 全绿：**109 个测试**（course-plan lib 65 + convergence 14 + icons 1 + render 3 + server 1 + server 集成 14 + sync-proto 11；0 failed / 0 ignored）。theme::tests 15 个，新增/关键：
  - `theme_config_keys_match_vendor_schema`（键集 + alpha）
  - `direct_theme_change_preserves_all_semantic_fields`（全部 ThemeColor + tokens + overlay.a）
  - `wcag_aa_contrast_on_filled_buttons`（fill + 文字，亮/暗）
  - `wcag_aa_shell_nav_and_link_pairs`（DS-3：active nav ink on `list_active`、inactive nav on `sidebar`、`link/_hover/_active` on `panel`/`surface`，亮/暗）
  - `tag::tests::{badge_text_meets_aa_on_semantic_fills, badge_text_meets_aa_on_danger_and_warning, tag_text_meets_aa_on_tints, filled_chip_text_meets_aa_including_light_fills, text_on_fill_meets_aa_on_midtone_fills, readable_text_meets_aa_on_midtone_backgrounds}`
  - `palette::tests::palette_at_for_switches_with_theme`
  - `tests/render.rs::shell_element_ids_are_stable`（§6.8 六个稳定 id 在渲染树可查询）
- [x] `cargo clippy --workspace --all-targets -- -D warnings` 无 warning。
- [x] `cargo fmt --all --check` 无 diff。
- [x] 切换主题：顶栏 toggle 按钮点击后浅↔深色生效（dashboard-light.png vs dashboard-dark.png 明显不同），直接 `Theme::change` 也不会让 overlay 变不透明。
- [x] Sidebar nav 点击切换：nav id 保持 Phase 2 字符串；active 文字/图标为 `foreground`（11.0:1 / 12.9:1），3px 竖条与 `list_active` 底色保留 `primary` 强调。
- [x] First-login dialog：走 `overlay` + `first_login_prompt` 组件（test id `first-login:ok/cloud/local/cancel`），cancel 真正关闭。
- [x] Schedule 7 列：日列固定 `w(88px)` + 网格 `min_w(696)`（56 + 7×88 + 6×4）+ 水平滚动容器；1280 下左栏 708 ≥ 696，周日完整可见。1000×640 未做无头截图（harness 固定 1280×840），按宽度算式核对：内容区 1000−256−40 = 704，左栏 704−260−16 = 428 → 网格自身水平滚动，不再裁切最右列。
- [x] `card` `Default` 变体 hover → `border_color: border_strong`（`ThemeColor.input`）已实现，与 §3.7 一致。
- [x] 8 个课程色互不相同（`course_palette_hues_are_distinct`，亮/暗各 ≥6 个不同 hex；实际 8 个）。

---

## 9. 版本与变更

| 版本 | 日期 | 变更 |
|---|---|---|
| v1.0 | 2026-10-17 | Phase 3 封版（lane DS 产出） |
| v1.1 | 2026-10-17 | Gate 3 预检修复：色相统一走 `hsl_deg(度数)`（gpui `Hsla.h ∈ [0,1)`）；课程色板统一改为 **hex-canonical**（`hex_color()` 反算，删除手写 HSL 值；暗色「嫩芽」定为 `#BCDC72`）；`Theme::change` 不再覆盖自有调色板（安装自定义 ThemeConfig） |
| v1.2 | 2026-10-17 | Gate 3 修复（lane DS-2）：**hex 是唯一契约**（`hex_color()` 反算，删除 HSL 列与 `hsl_deg_a`）；亮色 canvas 回暖纸 `#FBF6EC`；饱和 fill 加深 + 白字 / 暗色提亮 + 深字，§5.1 列真实对比度并由测试锁定；`tag/badge/filled chip` 自适应选字色（去掉写死白字）；`ThemeConfig` 修 `progress.bar` 键、补 `chart.1..5`、alpha 以 `#RRGGBBAA` 保留（overlay/scrollbar/drop_target/list_active_border/table_active_border），`selection`/`list_active` α 取 vendor 上限内；JSON builder 改为 `default()` + 字段赋值；`gpui-kit` 精确锁 `=0.7.3`；组件对齐 §3（按钮 36/圆角 14、list_row 透明+hover、card Hero shadow_md / Outlined 2px / 标题 17、stat_tile min_h108+value32+icon18、empty_state 56/32+Sparkle、alert_dialog 唯一 id + 真 cancel、nav active primary、badge 语义色）；app.rs 接入 `first_login_prompt` 并删除重复枚举；页面转场 260ms 唯一 id（reduce-motion 安全）；测试 id 统一（brand/theme-toggle/quick-add/sync-pill/sidebar sync indicator 可点击）；schedule 日列固定 88px 宽 + 水平滚动修周日截断；暗色课程盘经 `palette_at_for` 可达 |
| v1.3 | 2026-10-17 | Gate 3 残余修复（lane DS-3）：**对比度**——active nav 文字/图标改 `foreground`（`list_active` 复合底上 11.00:1 / 12.94:1，保留 3px `primary` 竖条与底色）；亮色 `muted_foreground` `#7A6F60 → #726758`（sidebar 4.68 / surface 5.14 / panel 5.40；顺带把 sync pill 离线态 `muted` 上的 4.04 修到 4.54）；`button::link` 改用蓝色 `link/link_hover/link_active`（亮 `#2E6A99/#2A5F8A/#24537A`、暗 `#7FBFEE/#97CBF3/#6AABE0`，panel/surface 全 ≥4.5，不再用 `primary` 2.4:1）；`sidebar_accent_foreground` 对齐为 `foreground`；WCAG 测试新增 shell/nav/link 对；`text_on_fill`/`readable_text` 增加纯黑/纯白兜底（暖墨/纸白不足时保证 ≥4.58，修 `#777777` 中灰）；亮色 `warning` 提亮 `#8F6612 → #9A650F`（去泥；白字 4.95、panel 文字 4.83）。**文档**——§3.1.4 确立唯一规则「顶栏独占页面名 + `quick_add` 主 CTA，内容区只用 `section_title`/次级动作」并同步 §4.1–4.8（删除逐页 `page_header` + 主按钮描述）；§2.8 按 vendor 实际修正 `Size` = 12/14/16/24；§6.1 记录 12 个私有 `base.*` 色在直接 `Theme::change` 后回退 gpui 默认的已接受限制；§9 v1.1 表述修正。**组件/布局**——card `Default` 实现 hover `border_strong`；schedule 网格最小宽 `672 → 696`（56 + 7×88 + 6×4，修最右列裁切） |
| v1.4 | 2026-10-18 | Phase 4 收官整合（lane DS-4，跨泳道 findings 归并）：**交互**——`overlay()` 根节点内建 `occlude()`（`BlockMouse`）：点击不再穿透到底下的行/卡片，删除各页自建的 `overlay_click_guard()`（V2 发现）；**动效**——打卡弹跳提升为 `ui::motion::bounce_animation(theme)` + `bounce_pulse(delta)`（视图 `render` 拿不到 `&mut Window`，删除不可用的 `bounce_spring`），checkins 直接调用；**中文**——`run()` 与测试 Harness 调 `gpui_component::set_locale("zh-CN")`（日历月份/星期变中文）；**一致性**——8 页页面 padding 统一 `p_5`（20，删除 §3.1.3 未落地的 24 分档；schedule 696 数学按 40 总侧 padding 对齐，v2 lane 断言同步）；对话框表单统一 `field_row`（课程/安排/日程/打卡编辑器的 Select 内置上方标签、色板/星期 caption、备注 Textarea 全部左置 label，§3.14）；**文档**——§2.7/§3.16 柱形取色对齐（今日 `chart_2` 满色、其余 α0.45）、§4.4 记录「当天课程」side rail 有意保留、§5.5/§6.5 动效 API 同步、§6.8 新增重复 ElementId 规则与中文过滤器 id 示例、§7 记录两项已接受限制（模态只压内容区；删除确认状态在 GPUI `Global`） |

任何后续调整必须更新本表并同步 §6 API Freeze。