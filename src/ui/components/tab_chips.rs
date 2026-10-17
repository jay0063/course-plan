//! Segmented tabs / filter chips（详见 `docs/DESIGN.md` §3.5）。
//!
//! Phase 4 视图层的标准用法：
//! ```ignore
//! tab_chips(
//!     &theme,
//!     "course-filter",
//!     &[("全部", CourseFilter::All), ("校内", CourseFilter::School), ("补习", CourseFilter::Tutoring)],
//!     app.course_filter,
//!     |v, cx| { app.update(cx, |a, cx| { a.course_filter = v; cx.notify(); }); },
//! )
//! ```

use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::tokens::{RADIUS_LG, RADIUS_MD};

/// Tab chips：水平并排，外层 `rounded_lg border p:1` 容器；每项 `rounded_md`。
///
/// 由于 Rust 闭包捕获限制，`T` 仅作为标识；运行时由 `selected == value` 比较。
/// `on_change` 接收点击项的 value；Phase 4 视图在调用前需构造 `Rc<T>` 副本。
pub fn tab_chips<T, F>(
    theme: &Theme,
    id_prefix: &str,
    items: &[(&str, T)],
    selected: T,
    on_change: F,
) -> impl IntoElement
where
    T: Copy + PartialEq + 'static,
    F: Fn(T, &mut Window, &mut App) + Clone + 'static,
{
    let mut container = div()
        .flex()
        .items_center()
        .gap(px(2.))
        .p(px(2.))
        .rounded(RADIUS_LG)
        .border_1()
        .border_color(theme.border)
        .bg(theme.background);

    for (label, value) in items {
        let is_selected = selected == *value;
        let id = ElementId::from(format!("{}:{}", id_prefix, label));
        let on_change_clone = on_change.clone();
        let value_copy = *value;
        container = container.child(chip_cell(
            theme,
            id,
            label.to_string(),
            move |_e, _w, cx| on_change_clone(value_copy, _w, cx),
            is_selected,
        ));
    }

    container
}

fn chip_cell<F>(
    theme: &Theme,
    id: ElementId,
    label: String,
    on_click: F,
    is_selected: bool,
) -> impl IntoElement
where
    F: Fn(&ClickEvent, &mut Window, &mut App) + 'static,
{
    let bg = if is_selected {
        theme.popover
    } else {
        Hsla {
            a: 0.0,
            ..theme.foreground
        }
    };
    let text_color = if is_selected {
        theme.foreground
    } else {
        theme.muted_foreground
    };
    let text_weight = if is_selected {
        FontWeight(600.0)
    } else {
        FontWeight(500.0)
    };

    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .px(px(12.))
        .h(px(30.))
        .rounded(RADIUS_MD)
        .cursor_pointer()
        .bg(bg)
        .when(!is_selected, |d| d.hover(|s| s.bg(theme.list_hover)))
        .text_sm()
        .font_weight(text_weight)
        .text_color(text_color)
        .child(label)
        .on_click(on_click)
}
