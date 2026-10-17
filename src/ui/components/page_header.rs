//! 页面标题块（详见 `docs/DESIGN.md` §3.1.3 / §4 各页结构）。

use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;

/// 标准页面头：大标题 + 描述 + 右侧操作。
///
/// title `text_size 20 / weight 600`；subtitle `text_sm / muted_foreground`。
pub fn page_header(
    theme: &Theme,
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    actions: Option<impl IntoElement>,
) -> impl IntoElement {
    let title = title.into();
    let subtitle: Option<SharedString> = subtitle.map(Into::into);

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(px(20.))
                        .font_weight(FontWeight(700.0))
                        .text_color(theme.foreground)
                        .child(title),
                )
                .when_some(subtitle, |d, s| {
                    d.child(div().text_sm().text_color(theme.muted_foreground).child(s))
                }),
        )
        .when_some(actions, |d, a| d.child(div().flex_shrink_0().child(a)))
}
