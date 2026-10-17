//! 页面标题块（详见 `docs/DESIGN.md` §3.1.3 / §4 各页结构）。

use crate::app::Page;
use crate::gpui::component::Theme;
use crate::gpui::*;
use crate::prelude::*;
use crate::ui::tokens::text as text_tokens;

/// 标准页面头：大标题 + 描述 + 右侧操作。
///
/// **归属规则（§3.1.4）**：顶栏拥有页面名/副标题与全局 quick-add；
/// `page_header` 仅用于页面内容里的英雄区，不得重复顶栏标题或主 CTA。
pub fn page_header(
    theme: &Theme,
    title: impl Into<SharedString>,
    subtitle: Option<impl Into<SharedString>>,
    actions: Option<impl IntoElement>,
) -> impl IntoElement {
    let title = title.into();
    let subtitle: Option<SharedString> = subtitle.map(Into::into);

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .text_size(text_tokens::TITLE)
                        .font_weight(FontWeight(700.0))
                        .text_color(theme.foreground)
                        .child(title),
                )
                .when_some(subtitle, |d, s| {
                    d.child(
                        div()
                            .text_size(text_tokens::BODY_SM)
                            .text_color(theme.muted_foreground)
                            .child(s),
                    )
                }),
        )
        .when_some(actions, |d, a| d.child(div().flex_shrink_0().child(a)))
}

/// 顶栏副标题 helper（按 Page 与数据库生成中文副标题）。
///
/// Phase 4 视图可在顶栏调用此函数生成 contextual subtitle。
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
