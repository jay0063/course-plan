//! Overlay / Dialog / AlertDialog / First-login prompt（详见 `docs/DESIGN.md` §3.12-§3.13）。

use crate::app::Page;
use crate::gpui::component::Theme;
use crate::gpui::component::button::Button;
use crate::gpui::*;
use crate::prelude::*;
use crate::sync::ProbeSummary;
use crate::ui::components::button;
use crate::ui::tokens::RADIUS_2XL;

/// 全屏 backdrop + 居中容器。Phase 4 视图编辑器的承载层。
///
/// backdrop `foreground.with_alpha(0.32)`（亮色）/ `0.55`（暗色）。
/// 容器 `bg: panel`、`rounded_2xl` (28)、`shadow_lg`、padding 24。
pub fn overlay(theme: &Theme, content: impl IntoElement) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        .p(px(24.))
        .bg(theme.foreground.opacity(0.32))
        .child(
            div()
                .bg(theme.popover)
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
/// `ok_label` = "删除" 等；`ok_variant` = danger。
/// `on_ok` 返回 true 关闭窗口；返回 false 保留（保留由调用方决定）。
pub fn alert_dialog(
    theme: &Theme,
    title: impl Into<SharedString>,
    description: impl Into<SharedString>,
    ok_label: impl Into<SharedString>,
    cancel_label: impl Into<SharedString>,
    ok_variant: AlertVariant,
    on_ok: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let title = title.into();
    let description = description.into();
    let ok_btn: Button = match ok_variant {
        AlertVariant::Danger => button::danger("alert_dialog_title", ok_label, on_ok),
        AlertVariant::Primary => button::primary("alert_dialog_title", ok_label, on_ok),
    };
    let cancel_btn = button::secondary("alert_dialog_cancel", cancel_label, |_, _, _| {});
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
/// `probe`: 服务端摘要；`on_choice`: 用户点击回调，传递策略与窗口。
pub fn first_login_prompt(
    theme: &Theme,
    probe: &ProbeSummary,
    on_choice: impl Fn(FirstLoginChoice, &mut Window, &mut App) + Clone + 'static,
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
                .text_color(theme.accent)
                .child("云端内容"),
        )
        .child(list);

    let mut buttons_row = div().flex().gap(px(8.)).flex_wrap();
    for (label, choice, primary) in [
        ("合并（推荐）", FirstLoginChoice::Merge, true),
        ("云端覆盖本地", FirstLoginChoice::CloudWins, false),
        ("本地覆盖云端", FirstLoginChoice::LocalWins, false),
    ] {
        let id = ElementId::from(format!("first-login:{}", label));
        let on_choice_clone = on_choice.clone();
        let btn: Button = if primary {
            button::primary(id, label, move |_e, w, cx| on_choice_clone(choice, w, cx))
        } else {
            button::secondary(id, label, move |_e, w, cx| on_choice_clone(choice, w, cx))
        };
        buttons_row = buttons_row.child(btn);
    }

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

    dialog(
        theme,
        "首次登录：云端与本地都有数据",
        body,
        Some(buttons_row),
    )
}

/// 首登对话框三选一（UI 侧枚举）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FirstLoginChoice {
    Merge,
    CloudWins,
    LocalWins,
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

/// Page subtitle helper（按 Page 与数据库生成中文副标题）。
///
/// Phase 4 视图可以在顶栏调用此函数生成 contextual subtitle。
pub fn page_subtitle(page: Page, db: &crate::model::Database) -> SharedString {
    use crate::model;
    let today = model::today();
    let weekday = model::iso_weekday(today);
    match page {
        Page::Dashboard => {
            let sessions = db.sessions_on(weekday).len();
            let checkins_today = db.checkins_on(today);
            let pending = checkins_today.iter().filter(|c| !c.done_on(today)).count();
            format!(
                "{} · 今日 {} 节课 · 待打卡 {} 项",
                model::fmt_date_cn(today),
                sessions,
                pending
            )
            .into()
        }
        Page::Courses => {
            let total = db.live_course_count();
            let archived = db.live_courses().filter(|c| !c.active).count();
            format!("已建 {} 门课程 · {} 项已归档", total, archived).into()
        }
        Page::Schedule => "按周重复的固定安排".into(),
        Page::Events => "本周与未来的额外安排".into(),
        Page::Reminders => {
            let stamp = model::now_string();
            let pending = db
                .live_reminders()
                .filter(|r| !r.done && crate::app::is_due(r.due_at.trim(), &stamp))
                .count();
            format!("{} 项待处理", pending).into()
        }
        Page::CheckIns => {
            let checkins_today = db.checkins_on(today);
            let done = checkins_today.iter().filter(|c| c.done_on(today)).count();
            format!("今日完成 {}/{}", done, checkins_today.len()).into()
        }
        Page::Memos => {
            let pinned = db.live_memos().filter(|m| m.pinned).count();
            format!("共 {} 条 · {} 已置顶", db.live_memo_count(), pinned).into()
        }
        Page::Settings => "个人信息与同步设置".into(),
    }
}
