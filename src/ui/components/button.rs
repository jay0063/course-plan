//! 按钮族（详见 `docs/DESIGN.md` §3.2）。
//!
//! 包含 `primary / secondary / ghost / danger / icon_only / link` 六种变体；
//! 每个变体强制设置正确的圆角（`rounded_lg`）、紧凑模式（`.compact()`）与
//! 颜色（gpui-kit ButtonVariants 自动按主题着色）。

use crate::gpui::assets::IconName;
use crate::gpui::component::button::{Button, ButtonVariants};
use crate::gpui::*;
use crate::prelude::*;

/// 主按钮：糖橙底 + 白字（或暗色下深棕字）。
pub fn primary(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .primary()
        .compact()
        .label(label)
        .on_click(on_click)
}

/// 主按钮 + 前置图标。
pub fn primary_with_icon(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: IconName,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .primary()
        .compact()
        .label(label)
        .icon(Icon::new(icon))
        .on_click(on_click)
}

/// 次级按钮：暖米底 + 棕字。
pub fn secondary(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .secondary()
        .compact()
        .label(label)
        .on_click(on_click)
}

/// 次级按钮 + 前置图标。
pub fn secondary_with_icon(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: IconName,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .secondary()
        .compact()
        .label(label)
        .icon(Icon::new(icon))
        .on_click(on_click)
}

/// Ghost 按钮：透明底，hover 时列表色。
pub fn ghost(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .ghost()
        .compact()
        .label(label)
        .on_click(on_click)
}

/// Ghost + 图标。
pub fn ghost_with_icon(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: IconName,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .ghost()
        .compact()
        .label(label)
        .icon(Icon::new(icon))
        .on_click(on_click)
}

/// 危险按钮（删除/清空）。
pub fn danger(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .danger()
        .compact()
        .label(label)
        .on_click(on_click)
}

/// 危险按钮 + 图标。
pub fn danger_with_icon(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    icon: IconName,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .danger()
        .compact()
        .label(label)
        .icon(Icon::new(icon))
        .on_click(on_click)
}

/// 图标按钮（顶栏主题切换等）：32×32 方形。
pub fn icon_only(
    id: impl Into<ElementId>,
    icon: IconName,
    tooltip: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .ghost()
        .compact()
        .icon(Icon::new(icon))
        .tooltip(tooltip)
        .on_click(on_click)
}

/// 链接按钮（更轻量的"查看全部"等内联动作）。
pub fn link(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .link()
        .compact()
        .label(label)
        .on_click(on_click)
}
