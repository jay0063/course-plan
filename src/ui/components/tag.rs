//! Tag / Badge / Chip（详见 `docs/DESIGN.md` §3.6）。

use crate::gpui::*;
use crate::prelude::*;
use crate::ui::tokens::RADIUS_FULL;

/// 圆角胶囊标签：用于卡片标题旁分类（"补习" / "校外"）。
///
/// 22px 高，padding-x 10px，bg = accent.with_alpha(0.18)，文字 12/600 accent。
pub fn tag(text: impl Into<SharedString>, accent: Hsla) -> impl IntoElement {
    let text = text.into();
    div()
        .flex()
        .items_center()
        .px(px(10.))
        .h(px(22.))
        .rounded_full()
        .bg(accent.with_alpha(0.18))
        .text_xs()
        .font_weight(FontWeight(600.0))
        .text_color(accent)
        .child(text)
}

/// 圆形徽章：用于导航 / 计数，18×18 圆点风格。
///
/// `count` 文字 11/700 白字，bg = accent。
pub fn badge(count: usize, accent: Hsla) -> impl IntoElement {
    let label = if count > 99 {
        "99+".to_string()
    } else {
        count.to_string()
    };
    div()
        .min_w(px(18.))
        .h(px(18.))
        .px(px(5.))
        .rounded_full()
        .bg(accent)
        .flex()
        .items_center()
        .justify_center()
        .text_size(px(11.))
        .font_weight(FontWeight(700.0))
        .text_color(Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 1.0,
        })
        .child(label)
}

/// Chip：更小的内联标签（20px 高）。
pub fn chip(text: impl Into<SharedString>, accent: Hsla) -> impl IntoElement {
    let text = text.into();
    div()
        .flex()
        .items_center()
        .px(px(8.))
        .h(px(20.))
        .rounded_full()
        .bg(accent.with_alpha(0.16))
        .text_size(px(11.))
        .font_weight(FontWeight(600.0))
        .text_color(accent)
        .child(text)
}

/// 旧版 `chip` 兼容（带 filled 选项）——views 已调用
/// `chip("...", color, true|false)`。filled=true 时为实心，false 为边框样式。
pub fn chip_with_filled(
    text: impl Into<SharedString>,
    color: Hsla,
    filled: bool,
) -> impl IntoElement {
    let text = text.into();
    let label_fg = if filled {
        Hsla {
            h: color.h,
            s: color.s,
            l: 0.99,
            a: 1.0,
        }
    } else {
        color
    };
    let mut d = div().px(px(8.)).py(px(2.)).rounded_full().text_xs();
    if filled {
        d = d.bg(color).text_color(label_fg);
    } else {
        d = d.border_1().border_color(color).text_color(color);
    }
    d.child(text)
}

// ensure RADIUS_FULL is referenced to silence unused import
#[allow(dead_code)]
const _: Pixels = RADIUS_FULL;
