//! Progress 与 Streak（详见 `docs/DESIGN.md` §3.10）。

use crate::gpui::assets::IconName;
use crate::gpui::component::Size;
use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::tokens::RADIUS_FULL;

/// 进度条：填充比例 0.0..=1.0，accent 色。
pub fn progress(theme: &Theme, value: f32, accent: Hsla, height: Pixels) -> impl IntoElement {
    let ratio = value.clamp(0.0, 1.0);
    div()
        .w_full()
        .h(height)
        .rounded(RADIUS_FULL)
        .bg(theme.muted)
        .child(
            div()
                .h_full()
                .w(relative(ratio))
                .rounded(RADIUS_FULL)
                .bg(accent),
        )
}

/// Streak 徽章（火焰 + 连续 N 天）。
pub fn streak_badge(theme: &Theme, days: usize) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .gap(px(4.))
        .text_xs()
        .font_weight(FontWeight(700.0))
        .text_color(theme.warning)
        .child(Icon::new(IconName::Flame).with_size(Size::XSmall))
        .child(format!("连续 {days} 天"))
}
