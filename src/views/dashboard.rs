use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use crate::app::{App, Page, is_due};
use crate::model::{self, WEEKDAY_LABELS};
use crate::ui::widgets::{card, check_circle, chip, eid, kv_row, palette_at, stat_tile, tint};

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let today = model::today();
    let weekday = model::iso_weekday(today);

    // MARK: 统计

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
        .flex()
        .gap_3()
        .flex_wrap()
        .child(stat_tile(
            &theme,
            "今日课程",
            format!("{} 节", today_sessions.len()),
            Some(format!("{} 分钟", today_minutes)),
            theme.primary,
            Some(IconName::BookOpen),
        ))
        .child(stat_tile(
            &theme,
            "课程总数",
            format!("{}", app.db.live_course_count()),
            Some(format!("校内 {} · 补习 {}", school_count, tutoring_count)),
            theme.info,
            Some(IconName::GraduationCap),
        ))
        .child(stat_tile(
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
        ))
        .child(stat_tile(
            &theme,
            "待办提醒",
            format!("{}", pending_reminders),
            Some(format!("备忘 {} 条", app.db.live_memo_count())),
            theme.warning,
            Some(IconName::Bell),
        ));

    // MARK: 今日课程时间轴

    let this = cx.entity();
    let mut timeline = div().flex().flex_col().gap_2();
    if today_sessions.is_empty() {
        timeline = timeline.child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("今天没有安排课程，安心休息"),
        );
    }
    for s in &today_sessions {
        let course = app.db.course(&s.course_id);
        let color = course.map(|c| palette_at(c.color)).unwrap_or(theme.primary);
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
            Some(chip("进行中", theme.success, true).into_any_element())
        } else if ended {
            Some(chip("已结束", theme.muted_foreground, false).into_any_element())
        } else {
            None
        };

        timeline = timeline.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .py_2()
                .rounded_md()
                .bg(if ongoing {
                    tint(color, &theme)
                } else {
                    Hsla {
                        a: 0.0,
                        ..theme.foreground
                    }
                })
                .border_1()
                .border_color(if ongoing { color } else { theme.border })
                .child(div().w(px(4.)).h(px(30.)).rounded_full().bg(color))
                .child(
                    div().w(px(96.)).flex_shrink_0().child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(if ended {
                                theme.muted_foreground
                            } else {
                                theme.foreground
                            })
                            .child(format!("{} - {}", s.start, s.end)),
                    ),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_sm()
                                .font_medium()
                                .text_color(theme.foreground)
                                .child(name),
                        )
                        .when(!location.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(location),
                            )
                        }),
                )
                .when_some(status, |d, s| d.child(s)),
        );
    }

    // MARK: 今日打卡

    let mut checkin_list = div().flex().flex_col().gap_2();
    if checkins_total == 0 {
        checkin_list = checkin_list.child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("今天没有打卡任务"),
        );
    }
    for c in checkins_today {
        let done = c.done_on(today);
        let id = c.id.clone();
        let streak = c.streak();
        let view = this.clone();

        checkin_list = checkin_list.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .id(eid("dash-checkin", &c.id))
                .on_click(move |_e, _window, cx| {
                    view.update(cx, |this, cx| {
                        let was = this
                            .db
                            .live_checkins()
                            .find(|x| x.id == id)
                            .map(|x| x.done_on(today));
                        if let Some(was) = was {
                            this.db.set_checkin_done(&id, today, !was);
                        }
                        this.commit(_window, cx);
                    });
                })
                .child(check_circle(&theme, done, px(20.)))
                .child(
                    div().flex_1().min_w_0().child(
                        div()
                            .text_sm()
                            .text_color(if done {
                                theme.muted_foreground
                            } else {
                                theme.foreground
                            })
                            .child(c.title.clone()),
                    ),
                )
                .when(streak > 0, |d| {
                    d.child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .text_xs()
                            .text_color(theme.warning)
                            .child(Icon::new(IconName::Flame).with_size(Size::XSmall))
                            .child(format!("连续 {} 天", streak)),
                    )
                }),
        );
    }

    // MARK: 待办提醒

    let mut reminder_list = div().flex().flex_col().gap_2();
    let pending: Vec<_> = app
        .db
        .live_reminders()
        .filter(|r| !r.done && is_due(r.due_at.trim(), &stamp))
        .take(5)
        .cloned()
        .collect();

    if pending.is_empty() {
        reminder_list = reminder_list.child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child("没有到期的提醒，状态很好"),
        );
    }
    for r in &pending {
        let id = r.id.clone();
        let view = this.clone();
        reminder_list = reminder_list.child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .cursor_pointer()
                .hover(|s| s.bg(theme.list_hover))
                .id(eid("dash-reminder", &r.id))
                .on_click(move |_e, window, cx| {
                    view.update(cx, |this, cx| {
                        this.db.set_reminder_done(&id, true);
                        this.commit(window, cx);
                    });
                })
                .child(div().size(px(8.)).rounded_full().bg(
                    if r.repeat == crate::model::Repeat::Once {
                        theme.warning
                    } else {
                        theme.info
                    },
                ))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.foreground)
                                .child(r.title.clone()),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(format!("{} · {}", r.due_at, r.repeat.label())),
                        ),
                ),
        );
    }

    // MARK: 一周负荷条形图

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

    let mut chart = div().flex().items_end().gap_2().h(px(120.)).px_1();
    for (i, minutes) in week_load.iter().enumerate() {
        let day = i + 1;
        let ratio = *minutes as f32 / max_minutes as f32;
        let height = px(8.0 + (ratio * 92.0));
        let is_today = day as u8 == weekday;
        let color = if is_today {
            theme.primary
        } else {
            theme.primary.with_alpha(0.35)
        };
        let label = WEEKDAY_LABELS[i];
        chart =
            chart.child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .items_center()
                    .justify_end()
                    .gap_1()
                    .h_full()
                    .child(div().text_xs().text_color(theme.muted_foreground).child(
                        if *minutes > 0 {
                            format!("{}", minutes)
                        } else {
                            String::new()
                        },
                    ))
                    .child(div().w_full().h(height).rounded_sm().bg(color))
                    .child(
                        div()
                            .text_xs()
                            .text_color(if is_today {
                                theme.primary
                            } else {
                                theme.muted_foreground
                            })
                            .child(label),
                    ),
            );
    }

    let go_page = move |p: Page, this: Entity<App>| {
        move |_e: &ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
            this.update(cx, |this, cx| {
                this.page = p;
                cx.notify();
            });
        }
    };

    div()
        .flex()
        .flex_col()
        .size_full()
        .overflow_y_scrollbar()
        .p_5()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_2xl()
                        .font_semibold()
                        .text_color(theme.foreground)
                        .child(format!("{}，今天也要加油！", greeting())),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "{} · 今日 {} 节课 · 待打卡 {} 项",
                            model::fmt_date_cn(today),
                            today_sessions.len(),
                            checkins_total.saturating_sub(checkins_done)
                        )),
                ),
        )
        .child(stats)
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap_4()
                .child(card(
                    &theme,
                    "今日课程",
                    Some(format!(
                        "{} 共 {} 分钟",
                        WEEKDAY_LABELS[(weekday - 1) as usize],
                        today_minutes
                    )),
                    timeline,
                ))
                .child(card(
                    &theme,
                    "今日打卡",
                    Some(format!("已完成 {}/{}", checkins_done, checkins_total)),
                    checkin_list,
                )),
        )
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap_4()
                .child(card(&theme, "本周课时分布", Some("单位：分钟"), chart))
                .child(card(
                    &theme,
                    "待办提醒",
                    Some(format!("{} 项待处理", pending.len())),
                    div().flex().flex_col().gap_3().child(reminder_list).child(
                        Button::new("goto-reminders")
                            .ghost()
                            .compact()
                            .label("查看全部提醒")
                            .icon(Icon::new(IconName::ChevronRight))
                            .on_click(go_page(Page::Reminders, this.clone())),
                    ),
                )),
        )
        .child(
            div()
                .grid()
                .grid_cols(2)
                .gap_4()
                .child(card(
                    &theme,
                    "快捷入口",
                    None::<SharedString>,
                    div()
                        .flex()
                        .gap_2()
                        .flex_wrap()
                        .child(
                            Button::new("q-courses")
                                .secondary()
                                .compact()
                                .label("课程")
                                .icon(Icon::new(IconName::BookOpen))
                                .on_click(go_page(Page::Courses, this.clone())),
                        )
                        .child(
                            Button::new("q-schedule")
                                .secondary()
                                .compact()
                                .label("计划表")
                                .icon(Icon::new(IconName::CalendarDays))
                                .on_click(go_page(Page::Schedule, this.clone())),
                        )
                        .child(
                            Button::new("q-events")
                                .secondary()
                                .compact()
                                .label("日程")
                                .icon(Icon::new(IconName::Clock))
                                .on_click(go_page(Page::Events, this.clone())),
                        )
                        .child(
                            Button::new("q-checkins")
                                .secondary()
                                .compact()
                                .label("打卡")
                                .icon(Icon::new(IconName::ListTodo))
                                .on_click(go_page(Page::CheckIns, this.clone())),
                        )
                        .child(
                            Button::new("q-memos")
                                .secondary()
                                .compact()
                                .label("备忘")
                                .icon(Icon::new(IconName::NotebookPen))
                                .on_click(go_page(Page::Memos, this.clone())),
                        ),
                ))
                .child(card(
                    &theme,
                    "本学期概况",
                    None::<SharedString>,
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
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
                        )),
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
