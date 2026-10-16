use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::*;

/// 课程配色。索引与 `Course.color` 对应。
pub const PALETTE: [Hsla; 8] = [
    Hsla {
        h: 212.0,
        s: 0.78,
        l: 0.56,
        a: 1.0,
    },
    Hsla {
        h: 344.0,
        s: 0.72,
        l: 0.62,
        a: 1.0,
    },
    Hsla {
        h: 158.0,
        s: 0.62,
        l: 0.46,
        a: 1.0,
    },
    Hsla {
        h: 38.0,
        s: 0.88,
        l: 0.58,
        a: 1.0,
    },
    Hsla {
        h: 268.0,
        s: 0.64,
        l: 0.64,
        a: 1.0,
    },
    Hsla {
        h: 190.0,
        s: 0.72,
        l: 0.48,
        a: 1.0,
    },
    Hsla {
        h: 96.0,
        s: 0.52,
        l: 0.52,
        a: 1.0,
    },
    Hsla {
        h: 24.0,
        s: 0.78,
        l: 0.58,
        a: 1.0,
    },
];

pub fn palette_at(index: usize) -> Hsla {
    PALETTE[index % PALETTE.len()]
}

/// 页面底色：浅色用纯白，深色用稍亮的底。
pub fn surface(theme: &Theme) -> Hsla {
    if theme.is_dark() {
        Hsla {
            h: 240.0,
            s: 0.06,
            l: 0.10,
            a: 1.0,
        }
    } else {
        Hsla {
            h: 220.0,
            s: 0.20,
            l: 0.975,
            a: 1.0,
        }
    }
}

/// 卡片底色。
pub fn panel(theme: &Theme) -> Hsla {
    if theme.is_dark() {
        Hsla {
            h: 240.0,
            s: 0.05,
            l: 0.145,
            a: 1.0,
        }
    } else {
        Hsla {
            h: 0.0,
            s: 0.0,
            l: 1.0,
            a: 1.0,
        }
    }
}

/// 课程色块背景（半透明）。
pub fn tint(color: Hsla, theme: &Theme) -> Hsla {
    Hsla {
        a: if theme.is_dark() { 0.26 } else { 0.14 },
        ..color
    }
}

/// 卡片容器。
pub fn card(
    theme: &Theme,
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    content: impl IntoElement,
) -> impl IntoElement {
    let title = title.into();
    let subtitle = subtitle.map(Into::into);
    div()
        .flex()
        .flex_col()
        .gap_3()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(panel(theme))
        .child(
            div().flex().items_center().justify_between().gap_2().child(
                div()
                    .flex()
                    .flex_col()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(theme.foreground)
                            .child(title),
                    )
                    .when_some(subtitle, |d, s| {
                        d.child(div().text_xs().text_color(theme.muted_foreground).child(s))
                    }),
            ),
        )
        .child(content)
}

/// 页面区块标题。
pub fn section_title(
    theme: &Theme,
    text: impl Into<SharedString>,
    trailing: Option<AnyElement>,
) -> impl IntoElement {
    let text = text.into();
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .mb_1()
        .child(
            div()
                .text_lg()
                .font_semibold()
                .text_color(theme.foreground)
                .child(text),
        )
        .when_some(trailing, |d, t| d.child(t))
}

/// 统计小卡片。
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
    div()
        .min_w(px(148.))
        .flex_1()
        .flex()
        .flex_col()
        .gap_1()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(panel(theme))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(label),
                )
                .when_some(icon, |d, i| {
                    d.child(
                        div()
                            .text_color(accent)
                            .child(Icon::new(i).with_size(Size::Small)),
                    )
                }),
        )
        .child(
            div()
                .text_2xl()
                .font_semibold()
                .text_color(theme.foreground)
                .child(value),
        )
        .when_some(hint, |d, h| {
            d.child(div().text_xs().text_color(theme.muted_foreground).child(h))
        })
}

/// 空状态。
pub fn empty_state(
    theme: &Theme,
    icon: IconName,
    title: impl Into<SharedString>,
    desc: Option<impl Into<SharedString>>,
) -> impl IntoElement {
    let title = title.into();
    let desc: Option<SharedString> = desc.map(Into::into);
    div()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_2()
        .py_10()
        .px_4()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .child(
            div()
                .text_color(theme.muted_foreground)
                .child(Icon::new(icon).with_size(Size::Large)),
        )
        .child(
            div()
                .text_sm()
                .font_medium()
                .text_color(theme.foreground)
                .child(title),
        )
        .when_some(desc, |d, s| {
            d.child(div().text_xs().text_color(theme.muted_foreground).child(s))
        })
}

/// 小圆角标签。
pub fn chip(text: impl Into<SharedString>, color: Hsla, filled: bool) -> impl IntoElement {
    let text = text.into();
    div()
        .px_2()
        .py(px(2.))
        .rounded_full()
        .text_xs()
        .when(filled, |d| {
            d.bg(color).text_color(Hsla {
                h: color.h,
                s: color.s,
                l: 0.99,
                a: 1.0,
            })
        })
        .when(!filled, |d| {
            d.border_1().border_color(color).text_color(color)
        })
        .child(text)
}

/// 带主题的键值行。
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
        .gap_3()
        .py_1()
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(key),
        )
        .child(
            div()
                .text_sm()
                .font_medium()
                .text_color(theme.foreground)
                .child(value),
        )
}

/// 组合元素 id。GPUI 的 `ElementId` 没有通用的元组转换，这里统一拼接。
pub fn eid(prefix: &str, key: &str) -> ElementId {
    ElementId::from(format!("{prefix}:{key}"))
}

/// 关闭编辑器遮罩。`pending` 指向宿主 App 上对应的编辑框槽位。
pub fn close_editor(app: Entity<crate::app::App>, cx: &mut gpui::App) {
    app.update(cx, |app, cx| {
        app.pending_course = None;
        app.pending_session = None;
        app.pending_event = None;
        app.pending_reminder = None;
        app.pending_checkin = None;
        app.pending_memo = None;
        cx.notify();
    });
}

/// 行内操作按钮。用文字而不是图标：图标在纯图标控件里不绘制，
/// 而且文字对家长更直观、对读屏软件也更友好。
pub fn action_button(
    id: ElementId,
    label: impl Into<SharedString>,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
) -> impl IntoElement {
    use gpui::component::button::ButtonVariants as _;
    gpui::component::button::Button::new(id)
        .ghost()
        .compact()
        .label(label)
        .on_click(on_click)
}

/// 完成状态的圆形标记。已完成时用实心圆 + 勾，未完成时是空心圆。
pub fn check_circle(theme: &Theme, done: bool, size: Pixels) -> impl IntoElement {
    div()
        .size(size)
        .rounded_full()
        .border_2()
        .border_color(if done { theme.success } else { theme.border })
        .bg(if done {
            theme.success
        } else {
            Hsla {
                a: 0.0,
                ..theme.foreground
            }
        })
        .flex_shrink_0()
        .when(done, |d| {
            d.child(div().w(px(6.)).h(px(6.)).rounded_full().bg(Hsla {
                h: 140.0,
                s: 0.6,
                l: 0.99,
                a: 1.0,
            }))
        })
}

/// 应用配色。把一组派生颜色打包，视图函数就不用传一长串 `Hsla`。
pub struct Skin {
    pub surface: Hsla,
    pub panel: Hsla,
    pub sidebar: Hsla,
    pub border: Hsla,
    pub fg: Hsla,
    pub muted: Hsla,
    pub primary: Hsla,
    pub primary_fg: Hsla,
}

impl Skin {
    pub fn new(theme: &Theme) -> Self {
        Self {
            surface: surface(theme),
            panel: panel(theme),
            sidebar: if theme.is_dark() {
                Hsla {
                    h: 240.0,
                    s: 0.06,
                    l: 0.085,
                    a: 1.0,
                }
            } else {
                Hsla {
                    h: 224.0,
                    s: 0.24,
                    l: 0.955,
                    a: 1.0,
                }
            },
            border: theme.border,
            fg: theme.foreground,
            muted: theme.muted_foreground,
            primary: theme.primary,
            primary_fg: theme.primary_foreground,
        }
    }
}
