//! Stat tile（详见 `docs/DESIGN.md` §3.8）。
//!
//! dashboard 顶部 4 块统计卡。Hero 风格卡片 + label/value/hint/icon。

use crate::gpui::assets::IconName;
use crate::gpui::component::Size;
use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::components::panel as panel_helper;
use crate::ui::tokens::text as text_tokens;
use crate::ui::tokens::{RADIUS_XL, SPACE_LG};

/// `card_hero` 风格统计卡：min_h 108、value 32/700、icon 18。
pub fn stat_tile(
    theme: &Theme,
    label: impl Into<SharedString>,
    value: impl Into<SharedString>,
    hint: Option<impl Into<SharedString>>,
    accent: Hsla,
    icon: Option<IconName>,
) -> impl IntoElement {
    let label = label.into();
    let value = value.into();
    let hint: Option<SharedString> = hint.map(Into::into);

    let mut head = div().flex().items_center().justify_between();
    head = head.child(
        div()
            .text_size(text_tokens::CAPTION)
            .text_color(theme.muted_foreground)
            .child(label),
    );
    if let Some(icon_name) = icon {
        head = head.child(
            div()
                .text_color(accent)
                .child(Icon::new(icon_name).with_size(Size::Size(px(18.)))),
        );
    }

    div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .min_w(px(148.))
        .min_h(px(108.))
        .flex_1()
        .p(SPACE_LG)
        .rounded(RADIUS_XL)
        .border_1()
        .border_color(theme.border)
        .bg(panel_helper::panel(theme))
        .child(head)
        .child(
            div()
                .text_size(text_tokens::STAT)
                .font_weight(FontWeight(700.0))
                .text_color(theme.foreground)
                .child(value),
        )
        .when_some(hint, |d, h| {
            d.child(
                div()
                    .text_size(text_tokens::CAPTION)
                    .text_color(theme.muted_foreground)
                    .child(h),
            )
        })
}
