//! List row（详见 `docs/DESIGN.md` §3.9）。
//!
//! 高度 auto、最小 48，gap 12；hover/active 状态。

use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::tokens::RADIUS_MD;

/// 列表行：leading [title [subtitle]] trailing。
///
/// 点击整行触发 `on_click`（如有）。`id` 用于交互测试定位（建议每行唯一）。
pub fn list_row(
    theme: &Theme,
    id: impl Into<ElementId>,
    on_click: Option<impl Fn(&ClickEvent, &mut Window, &mut App) + 'static>,
    leading: Option<impl IntoElement>,
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    trailing: Option<impl IntoElement>,
) -> impl IntoElement {
    let title = title.into();
    let subtitle: Option<SharedString> = subtitle.map(Into::into);

    let mut row = div()
        .id(id) // 强制 Stateful<Div>，确保 .on_click() 可用
        .flex()
        .items_center()
        .gap(px(12.))
        .min_h(px(48.))
        .px(px(12.))
        .rounded(RADIUS_MD)
        .border_1()
        .border_color(theme.border)
        .bg(theme.popover);

    if let Some(on_click) = on_click {
        row = row.on_click(on_click);
    }

    if let Some(leading) = leading {
        row = row.child(leading);
    }

    let text_block = div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(0.))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight(500.0))
                .text_color(theme.foreground)
                .child(title),
        )
        .when_some(subtitle, |d, s| {
            d.child(div().text_xs().text_color(theme.muted_foreground).child(s))
        });
    row = row.child(text_block);

    if let Some(trailing) = trailing {
        row = row.child(div().flex_shrink_0().child(trailing));
    }

    row
}
