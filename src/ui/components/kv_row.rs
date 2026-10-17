//! 键值行（详见 `docs/DESIGN.md` §3.9 衍生）。

use crate::gpui::component::Theme;
use crate::gpui::*;

/// 简单键值行：左侧 label、右侧 value。
pub fn kv_row(
    theme: &Theme,
    key: impl Into<SharedString>,
    value: impl Into<SharedString>,
) -> impl IntoElement {
    let key = key.into();
    let value = value.into();
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .py(px(4.))
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(key),
        )
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight(500.0))
                .text_color(theme.foreground)
                .child(value),
        )
}
