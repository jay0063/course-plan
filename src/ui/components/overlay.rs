//! Overlay / Dialog / AlertDialog / First-login prompt（详见 `docs/DESIGN.md` §3.12-§3.13）。

use crate::gpui::component::Theme;
use crate::gpui::component::button::Button;
use crate::gpui::*;
use crate::prelude::*;
use crate::sync::ProbeSummary;
use crate::ui::components::button;
use crate::ui::components::panel as panel_helper;
use crate::ui::tokens::RADIUS_2XL;

/// 全屏 backdrop + 居中容器。Phase 4 视图编辑器的承载层。
///
/// backdrop 直接用 `theme.overlay`（亮色 foreground α0.32 / 暗色 α0.55）——
/// `ThemeConfig` 也以 `#RRGGBBAA` 保存同一 alpha，所以直接 `Theme::change`
/// 后遮罩不会变成不透明色块。
pub fn overlay(theme: &Theme, content: impl IntoElement) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p(px(24.))
        .bg(theme.overlay)
        .child(
            div()
                .bg(panel_helper::panel(theme))
                .rounded(RADIUS_2XL)
                .p(px(24.))
                .min_w(px(420.))
                .max_w(px(560.))
                .shadow(vec![BoxShadow {
                    color: Hsla {
                        h: 0.0,
                        s: 0.0,
                        l: 0.0,
                        a: 0.20,
                    },
                    offset: point(px(0.), px(12.)),
                    spread_radius: px(0.),
                    blur_radius: px(24.),
                    inset: false,
                }])
                .border_1()
                .border_color(theme.border)
                .child(content),
        )
}

/// Dialog（标题 + body + footer）。
pub fn dialog(
    theme: &Theme,
    title: impl Into<SharedString>,
    body: impl IntoElement,
    footer: Option<impl IntoElement>,
) -> impl IntoElement {
    let title = title.into();
    let mut col = div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(
            div()
                .text_size(px(17.))
                .font_weight(FontWeight(600.0))
                .text_color(theme.foreground)
                .child(title),
        )
        .child(body);
    if let Some(footer) = footer {
        col = col.child(div().flex().justify_end().gap(px(8.)).child(footer));
    }
    col
}

/// AlertDialog：删除/清空二次确认。
///
/// `id_prefix` 用于生成唯一按钮 id（`{prefix}:ok` / `{prefix}:cancel`），
/// 避免多个对话框共用同一 id。`on_cancel` 必须真正关闭调用方的对话框。
#[allow(clippy::too_many_arguments)]
pub fn alert_dialog(
    theme: &Theme,
    id_prefix: &str,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_label: impl Into<SharedString>,
    cancel_label: impl Into<SharedString>,
    ok_variant: AlertVariant,
    on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    on_cancel: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let title = title.into();
    let description = description.into();
    let ok_btn: Button = match ok_variant {
        AlertVariant::Danger => button::danger(format!("{id_prefix}:ok"), ok_label, on_ok),
        AlertVariant::Primary => button::primary(format!("{id_prefix}:ok"), ok_label, on_ok),
    };
    let cancel_btn = button::secondary(format!("{id_prefix}:cancel"), cancel_label, on_cancel);
    dialog(
        theme,
        title,
        div()
            .text_sm()
            .text_color(theme.muted_foreground)
            .child(description),
        Some(div().flex().gap(px(8.)).child(cancel_btn).child(ok_btn)),
    )
}

#[derive(Clone, Copy)]
pub enum AlertVariant {
    Danger,
    Primary,
}

/// 首登三选一对话框（详见 §3.13）。
///
/// `probe`: 服务端摘要；`on_choice`: 用户点击回调。
/// 稳定 id：`first-login:ok`（合并）、`first-login:cloud`、`first-login:local`、
/// `first-login:cancel`（稍后决定）。
pub fn first_login_prompt(
    theme: &Theme,
    probe: &ProbeSummary,
    on_choice: impl Fn(FirstLoginAction, &mut Window, &mut App) + Clone + 'static,
) -> impl IntoElement {
    // 上半：云端内容
    let counts: Vec<String> = if probe.counts_by_kind.is_empty() {
        vec!["（云端暂无实体）".to_string()]
    } else {
        probe
            .counts_by_kind
            .iter()
            .map(|(kind, count)| format!("{} × {}", kind.label_cn(), count))
            .collect()
    };

    let mut list = div().flex().flex_col().gap(px(2.)).py(px(4.));
    for line in counts {
        list = list.child(div().text_sm().text_color(theme.foreground).child(line));
    }

    let counts_card = div()
        .flex()
        .flex_col()
        .gap(px(4.))
        .p(px(12.))
        .rounded(px(10.))
        .bg(theme.accent.with_alpha(0.10))
        .border_1()
        .border_color(theme.accent.with_alpha(0.4))
        .child(
            div()
                .text_xs()
                .font_weight(FontWeight(600.0))
                .text_color(crate::ui::components::tag::readable_text(
                    crate::ui::components::tag::composite(
                        theme.accent.with_alpha(0.10),
                        theme.popover,
                    ),
                    theme.accent,
                ))
                .child("云端内容"),
        )
        .child(list);

    let mut buttons_row = div().flex().items_center().gap(px(8.)).flex_wrap();
    for (id, label, action, primary) in [
        (
            "first-login:ok",
            "合并（推荐）",
            FirstLoginAction::Merge,
            true,
        ),
        (
            "first-login:cloud",
            "云端覆盖本地",
            FirstLoginAction::CloudWins,
            false,
        ),
        (
            "first-login:local",
            "本地覆盖云端",
            FirstLoginAction::LocalWins,
            false,
        ),
    ] {
        let on_choice_clone = on_choice.clone();
        let btn: Button = if primary {
            button::primary(id, label, move |_e, w, cx| on_choice_clone(action, w, cx))
        } else {
            button::secondary(id, label, move |_e, w, cx| on_choice_clone(action, w, cx))
        };
        buttons_row = buttons_row.child(btn);
    }
    let on_cancel = on_choice.clone();
    buttons_row = buttons_row.child(button::link(
        "first-login:cancel",
        "稍后决定",
        move |_e, w, cx| on_cancel(FirstLoginAction::Cancel, w, cx),
    ));

    let body = div()
        .flex()
        .flex_col()
        .gap(px(16.))
        .child(counts_card)
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child("「本地覆盖云端」会为云端独有数据推送删除（墓碑）。"),
        );

    div().w(px(480.)).child(dialog(
        theme,
        "首次登录：云端与本地都有数据",
        body,
        Some(buttons_row),
    ))
}

/// 首登对话框动作（UI 侧枚举，避免与核心类型耦合过紧）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstLoginAction {
    Merge,
    CloudWins,
    LocalWins,
    /// 先不决定，关闭对话框；同步策略留待下次登录。
    Cancel,
}

/// Section title（兼容旧 `ui::widgets::section_title` 签名）。
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
        .gap(px(12.))
        .mb(px(4.))
        .child(
            div()
                .text_size(px(20.))
                .font_weight(FontWeight(600.0))
                .text_color(theme.foreground)
                .child(text),
        )
        .when_some(trailing, |d, t| d.child(t))
}
