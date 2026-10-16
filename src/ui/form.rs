use gpui::prelude::*;
use gpui::*;

/// 表单里的一行：左边固定宽度标签，右边控制。
pub fn field_row(label: impl Into<SharedString>, control: impl IntoElement) -> impl IntoElement {
    let label = label.into();
    div()
        .flex()
        .items_start()
        .gap_3()
        .child(
            div()
                .w(px(84.))
                .flex_shrink_0()
                .pt(px(6.))
                .text_xs()
                .child(label),
        )
        .child(div().flex_1().min_w_0().child(control))
}
