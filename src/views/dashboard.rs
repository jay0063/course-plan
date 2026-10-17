//! 今日概览（DESIGN §4.1）。
//!
//! 结构：问候语 → 4 个 `stat_tile` → 今日课程 / 今日打卡 → 本周课时分布 /
//! 待办提醒 → 快捷入口 / 本学期概况。页面名与副标题只在顶栏（§3.1.4），
//! 内容区不重复；问候语按时辰切换，属内容而非标题。

use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use crate::app::{App, Page, is_due};
use crate::model::{self, WEEKDAY_LABELS};
use crate::ui::components::button;
use crate::ui::components::card::{CardVariant, card_with_theme};
use crate::ui::components::empty_state::{EmptyVariant, empty_state};
use crate::ui::components::kv_row::kv_row;
use crate::ui::components::list_row::list_row;
use crate::ui::components::progress::streak_badge;
use crate::ui::components::stat_tile::stat_tile;
use crate::ui::components::tag::chip;
use crate::ui::palette::{palette_at_for, tint};
use crate::ui::tokens::text as text_tokens;
use crate::ui::tokens::{
    RADIUS_FULL, RADIUS_MD, RADIUS_XL, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XL, SPACE_XS,
};
use crate::ui::widgets::eid;
use crate::views::checkins::check_burst;

/// 柱形图几何（§3.16 保留手写图表；高度按比例算，属 §1.2「一次性像素」例外）。
const CHART_H: f32 = 120.;
const CHART_BAR_MIN: f32 = 8.;
const CHART_BAR_MAX: f32 = 92.;

/// 可点击行包装。
///
/// `list_row` 的 opaque 返回类型上挂不了 `.test_support()`，所以把
/// id / 点击 / 测试观测放在外层 div 上（无视觉变化），列表行本身保持无交互。
fn clickable_row(
    id: ElementId,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut gpui::App) + 'static,
    child: impl IntoElement,
) -> impl IntoElement {
    div()
        .id(id)
        .cursor_pointer()
        .on_click(on_click)
        .test_support()
        .child(child)
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let today = model::today();
    let weekday = model::iso_weekday(today);
    let this = cx.entity();

    // MARK: 统计（与 Phase 2 完全一致的四个数字与提示）

    let today_sessions: Vec<_> = app.db.sessions_on(weekday).into_iter().cloned().collect();
    let today_minutes: i64 = today_sessions
        .iter()
        .map(|s| (s.end_minutes() - s.start_minutes()).max(0) as i64)
        .sum();
    let school_count = app
        .db
        .live_courses()
        .filter(|c| c.kind == crate::model::CourseKind::School)
        .count();
    let tutoring_count = app
        .db
        .live_courses()
        .filter(|c| c.kind == crate::model::CourseKind::Tutoring)
        .count();
    let now = model::now_minutes();

    let checkins_today = app.db.checkins_on(today);
    let checkins_done = checkins_today.iter().filter(|c| c.done_on(today)).count();
    let checkins_total = checkins_today.len();

    let stamp = model::now_string();
    let pending_reminders = app
        .db
        .live_reminders()
        .filter(|r| !r.done && is_due(r.due_at.trim(), &stamp))
        .count();

    let stats = div()
        .grid()
        .grid_cols(4)
        .gap(SPACE_LG)
        .child(div().rounded(RADIUS_XL).shadow_md().child(stat_tile(
            &theme,
            "今日课程",
            format!("{} 节", today_sessions.len()),
            Some(format!("{} 分钟", today_minutes)),
            theme.primary,
            Some(IconName::BookOpen),
        )))
        .child(div().rounded(RADIUS_XL).shadow_md().child(stat_tile(
            &theme,
            "课程总数",
            format!("{}", app.db.live_course_count()),
            Some(format!("校内 {} · 补习 {}", school_count, tutoring_count)),
            theme.info,
            Some(IconName::GraduationCap),
        )))
        .child(div().rounded(RADIUS_XL).shadow_md().child(stat_tile(
            &theme,
            "今日打卡",
            format!("{}/{}", checkins_done, checkins_total),
            Some(if checkins_total == 0 {
                "今天没有打卡任务".to_string()
            } else if checkins_done == checkins_total {
                "全部完成，很棒！".to_string()
            } else {
                format!("还剩 {} 项", checkins_total - checkins_done)
            }),
            theme.success,
            Some(IconName::Target),
        )))
        .child(div().rounded(RADIUS_XL).shadow_md().child(stat_tile(
            &theme,
            "待办提醒",
            format!("{}", pending_reminders),
            Some(format!("备忘 {} 条", app.db.live_memo_count())),
            theme.warning,
            Some(IconName::Bell),
        )));

    // MARK: 今日课程时间轴（课程色走 palette_at_for；进行中行带课程色淡底）

    let mut timeline = div().flex().flex_col().gap(SPACE_XS);
    if today_sessions.is_empty() {
        timeline = timeline.child(empty_state(
            &theme,
            EmptyVariant::Compact,
            IconName::CalendarDays,
            "今天没有课",
            Some("安心休息吧"),
            None::<AnyElement>,
        ));
    }
    for s in &today_sessions {
        let course = app.db.course(&s.course_id);
        let color = course
            .map(|c| palette_at_for(c.color, &theme))
            .unwrap_or(theme.primary);
        let name = course
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "未知课程".into());
        let location = if s.room.is_empty() {
            course.map(|c| c.location.clone()).unwrap_or_default()
        } else {
            s.room.clone()
        };
        let ended = now > s.end_minutes();
        let ongoing = now >= s.start_minutes() && now <= s.end_minutes();

        let status: Option<AnyElement> = if ongoing {
            Some(chip(&theme, "进行中", theme.success).into_any_element())
        } else if ended {
            Some(chip(&theme, "已结束", theme.muted_foreground).into_any_element())
        } else {
            None
        };
        let subtitle = if location.is_empty() {
            format!("{} - {}", s.start, s.end)
        } else {
            format!("{} - {} · {}", s.start, s.end, location)
        };

        let go_schedule = {
            let view = this.clone();
            move |_e: &ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
                view.update(cx, |this, cx| {
                    this.page = Page::Schedule;
                    cx.notify();
                });
            }
        };

        let row = clickable_row(
            eid("dash-session", &s.id),
            go_schedule,
            list_row(
                &theme,
                eid("dash-session-row", &s.id),
                None::<fn(&ClickEvent, &mut Window, &mut gpui::App)>,
                Some(div().w(px(4.)).h(px(30.)).rounded(RADIUS_FULL).bg(color)),
                name,
                Some(subtitle),
                status,
            ),
        );

        timeline = timeline.child(if ongoing {
            div()
                .rounded(RADIUS_MD)
                .bg(tint(color, &theme))
                .border_1()
                .border_color(color)
                .child(row)
                .into_any_element()
        } else {
            row.into_any_element()
        });
    }

    // MARK: 今日打卡（点击整行翻转 done，勾选圆环带 §3.17 弹跳）

    let mut checkin_list = div().flex().flex_col().gap(SPACE_XS);
    if checkins_total == 0 {
        checkin_list = checkin_list.child(empty_state(
            &theme,
            EmptyVariant::Compact,
            IconName::ListTodo,
            "今天没有打卡任务",
            Some("去「打卡任务」页新建一个"),
            None::<AnyElement>,
        ));
    }
    for c in checkins_today {
        let done = c.done_on(today);
        let id = c.id.clone();
        let streak = c.streak();
        let view = this.clone();

        checkin_list = checkin_list.child(clickable_row(
            eid("dash-checkin", &c.id),
            move |_e: &ClickEvent, window: &mut Window, cx: &mut gpui::App| {
                let id = id.clone();
                view.update(cx, |this, cx| {
                    let was = this
                        .db
                        .live_checkins()
                        .find(|x| x.id == id)
                        .map(|x| x.done_on(today));
                    if let Some(was) = was {
                        this.db.set_checkin_done(&id, today, !was);
                    }
                    this.commit(window, cx);
                });
            },
            list_row(
                &theme,
                eid("dash-checkin-row", &c.id),
                None::<fn(&ClickEvent, &mut Window, &mut gpui::App)>,
                Some(check_burst(
                    &theme,
                    done,
                    ElementId::from(format!("dash-checkin-burst:{}:{}", c.id, done)),
                )),
                c.title.clone(),
                None::<SharedString>,
                (streak > 0).then(|| streak_badge(&theme, streak)),
            ),
        ));
    }

    // MARK: 待办提醒

    let mut reminder_list = div().flex().flex_col().gap(SPACE_XS);
    let pending: Vec<_> = app
        .db
        .live_reminders()
        .filter(|r| !r.done && is_due(r.due_at.trim(), &stamp))
        .take(5)
        .cloned()
        .collect();

    if pending.is_empty() {
        reminder_list = reminder_list.child(empty_state(
            &theme,
            EmptyVariant::Compact,
            IconName::Bell,
            "没有到期的提醒",
            Some("今天可以喘口气"),
            None::<AnyElement>,
        ));
    }
    for r in &pending {
        let id = r.id.clone();
        let view = this.clone();
        reminder_list = reminder_list.child(clickable_row(
            eid("dash-reminder", &r.id),
            move |_e: &ClickEvent, window: &mut Window, cx: &mut gpui::App| {
                let id = id.clone();
                view.update(cx, |this, cx| {
                    this.db.set_reminder_done(&id, true);
                    this.commit(window, cx);
                });
            },
            list_row(
                &theme,
                eid("dash-reminder-row", &r.id),
                None::<fn(&ClickEvent, &mut Window, &mut gpui::App)>,
                Some(div().size(px(8.)).rounded(RADIUS_FULL).bg(
                    if r.repeat == crate::model::Repeat::Once {
                        theme.warning
                    } else {
                        theme.info
                    },
                )),
                r.title.clone(),
                Some(format!("{} · {}", r.due_at, r.repeat.label())),
                None::<AnyElement>,
            ),
        ));
    }

    let this_reminders = this.clone();
    let goto_reminders = button::ghost_with_icon(
        "dash-goto-reminders",
        "查看全部提醒",
        IconName::ChevronRight,
        move |_e, _w, cx| {
            this_reminders.update(cx, |this, cx| {
                this.page = Page::Reminders;
                cx.notify();
            });
        },
    );

    // MARK: 一周负荷柱形图（§3.16：chart_2 满色/α0.45，圆角 10，数值加粗）

    let mut max_minutes = 1;
    let week_load: Vec<i64> = (1..=7)
        .map(|d| {
            let m: i64 = app
                .db
                .sessions_on(d as u8)
                .iter()
                .map(|s| (s.end_minutes() - s.start_minutes()).max(0) as i64)
                .sum();
            max_minutes = max_minutes.max(m);
            m
        })
        .collect();

    let chart_color = theme.chart_2;
    let mut chart = div()
        .flex()
        .items_end()
        .gap(SPACE_SM)
        .h(px(CHART_H))
        .px(SPACE_XS);
    for (i, minutes) in week_load.iter().enumerate() {
        let day = i + 1;
        let ratio = *minutes as f32 / max_minutes as f32;
        let height = px(CHART_BAR_MIN + ratio * CHART_BAR_MAX);
        let is_today = day as u8 == weekday;
        let label = WEEKDAY_LABELS[i];
        chart = chart.child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_end()
                .gap(SPACE_XS)
                .h_full()
                .child(
                    div()
                        .text_size(text_tokens::CAPTION)
                        .font_weight(if is_today {
                            FontWeight(600.)
                        } else {
                            FontWeight(400.)
                        })
                        .text_color(if is_today {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(if *minutes > 0 {
                            format!("{}", minutes)
                        } else {
                            String::new()
                        }),
                )
                .child(div().w_full().h(height).rounded(RADIUS_MD).bg(if is_today {
                    chart_color
                } else {
                    chart_color.with_alpha(0.45)
                }))
                .child(
                    div()
                        .text_size(text_tokens::CAPTION)
                        .font_weight(if is_today {
                            FontWeight(600.)
                        } else {
                            FontWeight(400.)
                        })
                        .text_color(if is_today {
                            theme.foreground
                        } else {
                            theme.muted_foreground
                        })
                        .child(label),
                ),
        );
    }

    // MARK: 快捷入口（次级按钮，切页）

    let quick_page = |p: Page, this: Entity<App>| {
        move |_e: &ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
            this.update(cx, |this, cx| {
                this.page = p;
                cx.notify();
            });
        }
    };

    let quick = div()
        .flex()
        .gap(SPACE_SM)
        .flex_wrap()
        .child(button::secondary_with_icon(
            eid("dash-quick", "courses"),
            "课程",
            IconName::BookOpen,
            quick_page(Page::Courses, this.clone()),
        ))
        .child(button::secondary_with_icon(
            eid("dash-quick", "schedule"),
            "计划表",
            IconName::CalendarDays,
            quick_page(Page::Schedule, this.clone()),
        ))
        .child(button::secondary_with_icon(
            eid("dash-quick", "events"),
            "日程",
            IconName::Clock,
            quick_page(Page::Events, this.clone()),
        ))
        .child(button::secondary_with_icon(
            eid("dash-quick", "checkins"),
            "打卡",
            IconName::ListTodo,
            quick_page(Page::CheckIns, this.clone()),
        ))
        .child(button::secondary_with_icon(
            eid("dash-quick", "memos"),
            "备忘",
            IconName::NotebookPen,
            quick_page(Page::Memos, this.clone()),
        ));

    // MARK: 本学期概况

    let overview = div()
        .flex()
        .flex_col()
        .gap(SPACE_XS)
        .child(kv_row(
            &theme,
            "学期开始",
            app.db.profile.term_start.clone(),
        ))
        .child(kv_row(&theme, "学期结束", app.db.profile.term_end.clone()))
        .child(kv_row(
            &theme,
            "每周总课时",
            format!("{} 分钟", app.db.weekly_minutes()),
        ))
        .child(kv_row(
            &theme,
            "课表条目",
            format!("{} 条", app.db.live_session_count()),
        ))
        .child(kv_row(
            &theme,
            "备忘条数",
            format!("{} 条", app.db.live_memo_count()),
        ));

    div()
        .flex()
        .flex_col()
        .size_full()
        .overflow_y_scrollbar()
        .p_5()
        .gap(SPACE_XL)
        // 问候语是内容（不是页面名，§4.1）；日期与统计只在顶栏副标题里出现。
        .child(
            div()
                .text_size(text_tokens::DISPLAY)
                .font_weight(FontWeight(700.))
                .text_color(theme.foreground)
                .child(format!("{}，今天也要加油！", greeting())),
        )
        .child(stats)
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap(SPACE_LG)
                .child(card_with_theme(
                    &theme,
                    CardVariant::Default,
                    Some("今日课程"),
                    Some(format!(
                        "{} 共 {} 分钟",
                        WEEKDAY_LABELS[(weekday - 1) as usize],
                        today_minutes
                    )),
                    timeline,
                ))
                .child(card_with_theme(
                    &theme,
                    CardVariant::Default,
                    Some("今日打卡"),
                    Some(format!("已完成 {}/{}", checkins_done, checkins_total)),
                    checkin_list,
                )),
        )
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap(SPACE_LG)
                .child(card_with_theme(
                    &theme,
                    CardVariant::Default,
                    Some("本周课时分布"),
                    Some("单位：分钟"),
                    chart,
                ))
                .child(card_with_theme(
                    &theme,
                    CardVariant::Default,
                    Some("待办提醒"),
                    Some(format!("{} 项待处理", pending.len())),
                    div()
                        .flex()
                        .flex_col()
                        .gap(SPACE_MD)
                        .child(reminder_list)
                        .child(goto_reminders),
                )),
        )
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap(SPACE_LG)
                .child(card_with_theme(
                    &theme,
                    CardVariant::Default,
                    Some("快捷入口"),
                    None::<SharedString>,
                    quick,
                ))
                .child(card_with_theme(
                    &theme,
                    CardVariant::Default,
                    Some("本学期概况"),
                    None::<SharedString>,
                    overview,
                )),
        )
}

fn greeting() -> &'static str {
    let hour = chrono::Local::now().hour();
    match hour {
        5..=10 => "早上好",
        11..=13 => "中午好",
        14..=17 => "下午好",
        18..=22 => "晚上好",
        _ => "夜深了",
    }
}

use chrono::Timelike;
