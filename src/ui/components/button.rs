//! 按钮族（详见 `docs/DESIGN.md` §3.2）。
//!
//! 包含 `primary / secondary / ghost / danger / icon_only / link` 六种变体。
//! 每个变体强制尺寸（§3.2：文字按钮 36px、icon-only 32×32、圆角 14）与
//! 颜色（gpui-kit ButtonVariants 按主题着色——ThemeConfig 保证 fg 对比度）。

use crate::gpui::assets::IconName;
use crate::gpui::component::button::{Button, ButtonVariants};
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::tokens::{BUTTON_H, ICON_BUTTON_H, RADIUS_LG, RADIUS_MD, RADIUS_SM};

/// 文字按钮统一尺寸（36px 高、左右 16、圆角 14）。
fn sized(button: Button, padding_x: f32) -> Button {
    button.h(BUTTON_H).px(px(padding_x)).rounded(RADIUS_LG)
}

/// 主按钮：糖橙底 + 深棕字（亮色）/ 深棕字（暗色，见 §2.2）。
pub fn primary(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    sized(Button::new(id).primary(), 16.)
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
    sized(Button::new(id).primary(), 16.)
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
    sized(Button::new(id).secondary(), 16.)
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
    sized(Button::new(id).secondary(), 16.)
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
    sized(Button::new(id).ghost(), 12.)
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
    sized(Button::new(id).ghost(), 12.)
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
    sized(Button::new(id).danger(), 16.)
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
    sized(Button::new(id).danger(), 16.)
        .label(label)
        .icon(Icon::new(icon))
        .on_click(on_click)
}

/// 图标按钮（顶栏主题切换等）：32×32、圆角 10。
pub fn icon_only(
    id: impl Into<ElementId>,
    icon: IconName,
    tooltip: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .ghost()
        .size(ICON_BUTTON_H)
        .rounded(RADIUS_MD)
        .icon(Icon::new(icon))
        .tooltip(tooltip)
        .on_click(on_click)
}

/// 链接按钮（更轻量的"查看全部"等内联动作）：24px 高。
pub fn link(
    id: impl Into<ElementId>,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Button {
    Button::new(id)
        .link()
        .h(px(24.))
        .px(px(4.))
        .rounded(RADIUS_SM)
        .label(label)
        .on_click(on_click)
}
