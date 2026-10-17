//! 侧栏导航项（详见 `docs/DESIGN.md` §3.3）。

use crate::gpui::assets::IconName;
use crate::gpui::component::Size;
use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::components::tag as tag_comp;
use crate::ui::tokens::NAV_ITEM_H;

/// 单个 nav item：icon + label + (optional badge)。
///
/// `active` 时高亮（primary 色），左缘 3px 圆角竖条。
pub fn nav_item(
    theme: &Theme,
    id: ElementId,
    icon: IconName,
    label: impl Into<SharedString>,
    active: bool,
    badge: Option<usize>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let label = label.into();
    let badge_color = if active { theme.warning } else { theme.danger };

    let icon_color = if active {
        theme.primary
    } else {
        theme.muted_foreground
    };
    let text_color = if active {
        theme.foreground
    } else {
        theme.muted_foreground
    };
    let text_weight = if active {
        FontWeight(600.0)
    } else {
        FontWeight(500.0)
    };

    let bg_color = if active {
        theme.list_active
    } else {
        Hsla {
            a: 0.0,
            ..theme.foreground
        }
    };
    let hover_bg = if active {
        theme.list_active
    } else {
        theme.list_hover
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .gap(px(8.))
        .h(NAV_ITEM_H)
        .pl(px(12.))
        .pr(px(10.))
        .my(px(1.))
        .rounded(px(10.))
        .cursor_pointer()
        .bg(bg_color)
        .hover(|s| s.bg(hover_bg))
        .on_click(on_click)
        // 左侧 3px 圆角竖条（active 时）
        .child(
            div()
                .w(px(3.))
                .h(px(20.))
                .flex_shrink_0()
                .rounded(px(2.))
                .when(active, |d| d.bg(theme.primary))
                .when(!active, |d| d.opacity(0.0)),
        )
        .child(
            div()
                .flex()
                .flex_1()
                .min_w_0()
                .items_center()
                .gap(px(8.))
                .child(
                    Icon::new(icon)
                        .with_size(Size::Small)
                        .text_color(icon_color)
                        .flex_shrink_0(),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .font_weight(text_weight)
                        .text_color(text_color)
                        .whitespace_nowrap()
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(label),
                ),
        )
        .when_some(badge, |d, b| {
            if b > 0 {
                d.child(div().flex_shrink_0().child(tag_comp::badge(b, badge_color)))
            } else {
                d
            }
        })
}
