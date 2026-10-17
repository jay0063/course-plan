//! List row（详见 `docs/DESIGN.md` §3.9）。
//!
//! 高度 auto、最小 48，gap 12；默认透明，hover/active 才出现底色。

use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::tokens::RADIUS_MD;

/// 列表行：leading [title [subtitle]] trailing。
///
/// 默认 bg `transparent`（放在卡片里不叠加第二层纸色）；hover `list_hover`；
/// 按下 `list_active`。点击整行触发 `on_click`（如有）。
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
        .id(id)
        .flex()
        .items_center()
        .gap(px(12.))
        .min_h(px(48.))
        .px(px(12.))
        .rounded(RADIUS_MD)
        .hover(|s| s.bg(theme.list_hover))
        .active(|s| s.bg(theme.list_active));

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
                .font_weight(FontWeight(600.0))
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
