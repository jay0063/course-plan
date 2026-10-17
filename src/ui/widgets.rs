//! **Back-compat 兼容层**（详见 `docs/DESIGN.md` §6.7）。
//!
//! Phase 3 期间，views 未迁移；它们仍然 `use crate::ui::widgets::{...}` 调用旧签名
//! helper。本模块把旧调用全部桥接到 `ui::components::...` 新组件 + `ui::palette::...`
//! 调色板，保证视图层一行不改即可通过编译。
//!
//! Phase 4 完成后此层将逐步被删除；任何新增 helper 必须直接写在 `ui::components`。

use crate::gpui::assets::IconName;
use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;

use crate::ui::components::card as card_new;
use crate::ui::components::check_circle as cc_new;
use crate::ui::components::empty_state as es_new;
use crate::ui::components::kv_row as kv_new;
use crate::ui::components::overlay::section_title as st_new;
use crate::ui::components::panel as panel_helper;
use crate::ui::components::stat_tile as st_tile;
use crate::ui::components::tag as tag_comp;
use crate::ui::palette as palette_pub;

// MARK: 调色板

/// 课程 8 色调色板（旧视图层引用）。等同 [`crate::ui::palette::COURSE_PALETTE`]。
pub const PALETTE: [Hsla; 8] = palette_pub::COURSE_PALETTE;

/// 取第 N 色（环绕）。
pub fn palette_at(index: usize) -> Hsla {
    palette_pub::palette_at(index)
}

/// 全屏底色（亮色米纸；暗色深棕纸）。
pub fn surface(theme: &Theme) -> Hsla {
    panel_helper::surface(theme)
}

/// 卡片/侧栏/顶栏底色。
pub fn panel(theme: &Theme) -> Hsla {
    panel_helper::panel(theme)
}

/// 课程色淡底。
pub fn tint(color: Hsla, theme: &Theme) -> Hsla {
    palette_pub::tint(color, theme)
}

// MARK: 组件（保持旧签名，内部委托到 ui::components）

/// 旧版 `card(theme, title, subtitle, content)` —— 内部等价
/// `ui::components::card::card_with_theme(Default, ...)`。
pub fn card(
    theme: &Theme,
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    content: impl IntoElement,
) -> impl IntoElement {
    card_new::card_with_theme(
        theme,
        card_new::CardVariant::Default,
        Some(title),
        subtitle,
        content,
    )
}

/// 旧版 `section_title(theme, text, trailing)`。
pub fn section_title(
    theme: &Theme,
    text: impl Into<SharedString>,
    trailing: Option<AnyElement>,
) -> impl IntoElement {
    st_new(theme, text, trailing)
}

/// 旧版 `stat_tile(theme, label, value, hint, accent, icon)`。
pub fn stat_tile(
    theme: &Theme,
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    hint: Option<impl Into<SharedString>>,
    accent: Hsla,
    icon: Option<IconName>,
) -> impl IntoElement {
    st_tile::stat_tile(theme, label, value, hint, accent, icon)
}

/// 旧版 `empty_state(theme, icon, title, desc)` —— 内部 Compact variant。
pub fn empty_state(
    theme: &Theme,
    icon: IconName,
    title: impl Into<SharedString>,
    description: Option<impl Into<SharedString>>,
) -> impl IntoElement {
    es_new::empty_state(
        theme,
        es_new::EmptyVariant::Compact,
        icon,
        title,
        description,
        None::<gpui::Div>,
    )
}

/// 旧版 `chip(text, color, filled)` —— 桥接到 `tag::chip_with_filled`。
pub fn chip(text: impl Into<SharedString>, color: Hsla, filled: bool) -> impl IntoElement {
    tag_comp::chip_with_filled(text, color, filled)
}

/// 旧版 `kv_row(theme, key, value)`。
pub fn kv_row(
    theme: &Theme,
    key: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> impl IntoElement {
    kv_new::kv_row(theme, key, value)
}

/// 旧版 `check_circle(theme, done, size)`。
pub fn check_circle(theme: &Theme, done: bool, size: Pixels) -> impl IntoElement {
    cc_new::check_circle(theme, done, size)
}

// MARK: 必要公共 API

/// 组合 ElementId（views 一直使用，测试 harness 也依赖）。
pub fn eid(prefix: &str, key: &str) -> ElementId {
    ElementId::from(format!("{prefix}:{key}"))
}

/// 关闭所有编辑器遮罩——测试 harness 依赖此函数（不可改名）。
pub fn close_editor(app: Entity<crate::app::App>, cx: &mut gpui::App) {
    app.update(cx, |app, cx| {
        app.pending_course = None;
        app.pending_session = None;
        app.pending_event = None;
        app.pending_reminder = None;
        app.pending_checkin = None;
        app.pending_memo = None;
        cx.notify();
    });
}

/// 旧版 action_button（ghost 变体 + label）—— 桥接到 components::button::ghost。
pub fn action_button(
    id: ElementId,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    crate::ui::components::button::ghost(id, label, on_click)
}

// MARK: 应用配色聚合（views/app.rs 仍可用）

/// 把派生颜色打包的便捷聚合——`app.rs::Skin` 仍然用此名访问这些字段。
pub struct Skin {
    pub surface: Hsla,
    pub panel: Hsla,
    pub sidebar: Hsla,
    pub border: Hsla,
    pub fg: Hsla,
    pub muted: Hsla,
    pub primary: Hsla,
    pub primary_fg: Hsla,
}

impl Skin {
    pub fn new(theme: &Theme) -> Self {
        Self {
            surface: panel_helper::surface(theme),
            panel: panel_helper::panel(theme),
            sidebar: theme.sidebar,
            border: theme.border,
            fg: theme.foreground,
            muted: theme.muted_foreground,
            primary: theme.primary,
            primary_fg: theme.primary_foreground,
        }
    }
}
