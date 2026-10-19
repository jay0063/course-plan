use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use super::widgets::{eid, panel};
use gpui::TestSupportExt;

/// 一个选项。
#[derive(Clone)]
pub struct SelectOption {
    pub label: SharedString,
    pub hint: Option<SharedString>,
    pub color: Option<Hsla>,
}

impl SelectOption {
    pub fn new(label: impl Into<SharedString>) -> Self {
        Self {
            label: label.into(),
            hint: None,
            color: None,
        }
    }

    pub fn hint(mut self, hint: impl Into<SharedString>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// 轻量下拉选择器。展开时在控件下方内联显示选项列表，
/// 这样在可滚动的表单里也能稳定定位，不依赖浮层定位。
pub struct Select {
    id: SharedString,
    label: Option<SharedString>,
    placeholder: SharedString,
    options: Vec<SelectOption>,
    selected: Option<usize>,
    open: bool,
}

impl Select {
    pub fn new(id: impl Into<SharedString>) -> Self {
        Self {
            id: id.into(),
            label: None,
            placeholder: "请选择".into(),
            options: Vec::new(),
            selected: None,
            open: false,
        }
    }

    pub fn label(mut self, label: impl Into<SharedString>) -> Self {
        self.label = Some(label.into());
        self
    }

    pub fn placeholder(mut self, text: impl Into<SharedString>) -> Self {
        self.placeholder = text.into();
        self
    }

    pub fn options(mut self, options: Vec<SelectOption>) -> Self {
        self.options = options;
        self
    }

    pub fn selected(mut self, index: Option<usize>) -> Self {
        self.selected = index;
        self
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }
}

impl Render for Select {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Copy the colors out so the `&Theme` borrow ends immediately.
        let (fg, muted, primary, primary_hover, border, list_hover, bg) = {
            let t = cx.theme();
            (
                t.foreground,
                t.muted_foreground,
                t.primary,
                t.primary_hover,
                t.border,
                t.list_hover,
                panel(t),
            )
        };

        let current = self.selected.and_then(|i| self.options.get(i)).cloned();
        let has_value = current.is_some();

        let mut trigger = div()
            .id(eid("select-trigger", &self.id))
            .flex()
            .items_center()
            .justify_between()
            .gap_2()
            .w_full()
            .h(px(32.))
            .px_3()
            .rounded_md()
            .border_1()
            .border_color(if self.open { primary } else { border })
            .bg(bg)
            .cursor_pointer()
            .test_support()
            .hover(|s| s.border_color(primary_hover))
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_2()
                    .overflow_hidden()
                    .child(
                        div()
                            .text_sm()
                            .text_color(if has_value { fg } else { muted })
                            .child(
                                current
                                    .as_ref()
                                    .map(|o| o.label.clone())
                                    .unwrap_or_else(|| self.placeholder.clone()),
                            ),
                    )
                    .when_some(current.as_ref().and_then(|o| o.color), |d, c| {
                        d.child(div().size(px(8.)).rounded_full().bg(c))
                    }),
            )
            .child(
                Icon::new(if self.open {
                    IconName::ChevronUp
                } else {
                    IconName::ChevronDown
                })
                .with_size(Size::XSmall)
                .text_color(muted),
            );

        let this = cx.entity();
        trigger = trigger.on_click(move |_e, _window, cx| {
            this.update(cx, |this, cx| {
                this.open = !this.open;
                cx.notify();
            });
        });

        let mut column = div().flex().flex_col().gap_1();

        if let Some(label) = self.label.clone() {
            column = column.child(div().text_xs().text_color(muted).child(label));
        }
        column = column.child(trigger);

        if self.open {
            let mut list = div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .mt_1()
                .p_1()
                .rounded_md()
                .border_1()
                .border_color(border)
                .bg(bg)
                .max_h(px(240.))
                .overflow_y_scrollbar();

            for (index, opt) in self.options.iter().enumerate() {
                let is_selected = self.selected == Some(index);
                let this = cx.entity();
                let label = opt.label.clone();
                let hint = opt.hint.clone();
                let dot = opt.color;

                list = list.child(
                    div()
                        .id(eid("select-option", &format!("{}:{}", self.id, index)))
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .px_2()
                        .py(px(6.))
                        .rounded_sm()
                        .cursor_pointer()
                        .test_support()
                        .bg(if is_selected {
                            primary.with_alpha(0.12)
                        } else {
                            Hsla { a: 0.0, ..fg }
                        })
                        .hover(|s| s.bg(list_hover))
                        .on_click(move |_e, _window, cx| {
                            this.update(cx, |this, cx| {
                                this.selected = Some(index);
                                this.open = false;
                                cx.notify();
                            });
                        })
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(div().text_sm().text_color(fg).child(label))
                                .when_some(dot, |d, c| {
                                    d.child(div().size(px(8.)).rounded_full().bg(c))
                                }),
                        )
                        .when_some(hint, |d, h| {
                            d.child(div().text_xs().text_color(muted).child(h))
                        }),
                );
            }
            column = column.child(list);
        }

        div().w_full().child(column)
    }
}
