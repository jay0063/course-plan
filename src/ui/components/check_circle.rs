//! 完成勾选圆形（详见 `docs/DESIGN.md` §3.7-§3.10）。

use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::palette::hsl_deg;

/// `check_circle`（已/未完成 圆形标记）。
///
/// `done=true` 时实心 success 色；`false` 时 2px 边框 + 透明底。
pub fn check_circle(theme: &Theme, done: bool, size: Pixels) -> impl IntoElement {
    let border_color = if done { theme.success } else { theme.border };
    let bg = if done {
        theme.success
    } else {
        Hsla {
            a: 0.0,
            ..theme.foreground
        }
    };
    div()
        .size(size)
        .rounded_full()
        .border_2()
        .border_color(border_color)
        .bg(bg)
        .flex_shrink_0()
        .when(done, |d| {
            // 内部小圆点：薄荷绿（hsl_deg 140°/0.6/0.99 ≈ light green）。
            d.child(
                div()
                    .w(px(6.))
                    .h(px(6.))
                    .rounded_full()
                    .bg(hsl_deg(140.0, 0.6, 0.99)),
            )
        })
}
