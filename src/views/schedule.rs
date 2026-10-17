use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::WindowExt;
use gpui::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use std::rc::Rc;

use crate::app::App;
use crate::model::{Database, Session, WEEKDAY_LABELS, WEEKDAY_SHORT};
use crate::ui::form::field_row;
use crate::ui::palette::palette_at_for;
use crate::ui::select::{Select, SelectOption};
use crate::ui::widgets::{
    action_button, card, close_editor, eid, empty_state, panel, section_title, tint,
};

#[derive(Clone)]
pub struct SessionDraft {
    pub id: String,
    pub course_id: String,
    pub weekday: u8,
    pub start: String,
    pub end: String,
    pub room: String,
}

impl SessionDraft {
    pub fn empty() -> Self {
        Self {
            id: String::new(),
            course_id: String::new(),
            weekday: model_today_weekday(),
            start: "08:00".into(),
            end: "08:45".into(),
            room: String::new(),
        }
    }

    pub fn from(session: &Session) -> Self {
        Self {
            id: session.id.clone(),
            course_id: session.course_id.clone(),
            weekday: session.weekday,
            start: session.start.clone(),
            end: session.end.clone(),
            room: session.room.clone(),
        }
    }
}

fn model_today_weekday() -> u8 {
    crate::model::iso_weekday(crate::model::today())
}

type SaveCb = Rc<dyn Fn(SessionDraft, &mut Window, &mut gpui::App)>;

pub struct SessionEditor {
    owner: Entity<App>,
    draft: SessionDraft,
    courses: Vec<(String, String)>,
    course: Entity<Select>,
    weekday: Entity<Select>,
    start: Entity<InputState>,
    end: Entity<InputState>,
    room: Entity<InputState>,
    note: Entity<TextareaState>,
    on_save: SaveCb,
}

impl SessionEditor {
    pub fn new(
        draft: &SessionDraft,
        courses: Vec<(String, String)>,
        window: &mut Window,
        cx: &mut Context<App>,
        on_save: impl Fn(SessionDraft, &mut Window, &mut gpui::App) + 'static,
    ) -> Entity<Self> {
        let owner = cx.entity();
        let options: Vec<SelectOption> = courses
            .iter()
            .map(|(_, name)| SelectOption::new(name.clone()))
            .collect();
        let selected = courses.iter().position(|(id, _)| *id == draft.course_id);

        let day_options: Vec<SelectOption> = WEEKDAY_LABELS
            .iter()
            .map(|label| SelectOption::new(*label))
            .collect();

        let start_text = draft.start.clone();
        let end_text = draft.end.clone();
        let room_text = draft.room.clone();

        let editor = Self {
            owner,
            draft: draft.clone(),
            courses,
            course: cx.new(|_| {
                Select::new("session-course")
                    .label("所属课程")
                    .placeholder("选择课程")
                    .options(options)
                    .selected(selected)
            }),
            weekday: cx.new(|_| {
                Select::new("session-weekday")
                    .label("星期")
                    .options(day_options)
                    .selected(Some(draft.weekday.saturating_sub(1) as usize))
            }),
            start: cx.new(|cx| InputState::new(window, cx).placeholder("08:00")),
            end: cx.new(|cx| InputState::new(window, cx).placeholder("08:45")),
            room: cx.new(|cx| InputState::new(window, cx).placeholder("教室 / 地点")),
            note: cx.new(|cx| TextareaState::new(window, cx).placeholder("备注")),
            on_save: Rc::new(on_save),
        };

        editor
            .start
            .update(cx, |s, cx| s.set_value(start_text, window, cx));
        editor
            .end
            .update(cx, |s, cx| s.set_value(end_text, window, cx));
        editor
            .room
            .update(cx, |s, cx| s.set_value(room_text, window, cx));

        cx.new(|_| editor)
    }

    fn snapshot(&self, cx: &gpui::App) -> SessionDraft {
        let course_id = self
            .course
            .read(cx)
            .selected_index()
            .and_then(|i| self.courses.get(i))
            .map(|(id, _)| id.clone())
            .unwrap_or_default();
        let weekday = self
            .weekday
            .read(cx)
            .selected_index()
            .map(|i| (i as u8).saturating_add(1).min(7))
            .unwrap_or(1);
        SessionDraft {
            id: self.draft.id.clone(),
            course_id,
            weekday,
            start: self.start.read(cx).value().to_string(),
            end: self.end.read(cx).value().to_string(),
            room: self.room.read(cx).value().to_string(),
        }
    }
}

impl Render for SessionEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();
        let course = self.course.clone();
        let weekday = self.weekday.clone();
        let start = self.start.clone();
        let this_close = this.clone();
        let end = self.end.clone();
        let room = self.room.clone();
        let note = self.note.clone();

        div()
            .w(px(460.))
            .p_5()
            .rounded_lg()
            .flex()
            .flex_col()
            .gap_3()
            .bg(panel(&theme))
            .border_1()
            .border_color(theme.border)
            .shadow_lg()
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(theme.foreground)
                    .child("排课"),
            )
            .child(course)
            .child(weekday)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(field_row("开始", Input::new(&start).h(px(32.)).w_full())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(field_row("结束", Input::new(&end).h(px(32.)).w_full())),
                    ),
            )
            .child(field_row("教室", Input::new(&room).h(px(32.)).w_full()))
            .child(
                div()
                    .w_full()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .child(Textarea::new(&note).w_full()),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .pt_2()
                    .child(
                        Button::new("session-cancel")
                            .secondary()
                            .compact()
                            .label("取消")
                            .on_click(move |_e, _w, app| {
                                close_editor(this_close.read(app).owner.clone(), app);
                            }),
                    )
                    .child(
                        Button::new("session-save")
                            .primary()
                            .compact()
                            .label("保存")
                            .on_click(move |_e, window, app| {
                                let (draft, cb) = {
                                    let guard = this.read(app);
                                    (guard.snapshot(app), guard.on_save.clone())
                                };
                                cb(draft, window, app);
                            }),
                    ),
            )
    }
}

pub fn apply_draft(db: &mut Database, draft: &SessionDraft) {
    let mut session = match db.sessions.iter().find(|s| s.id == draft.id) {
        Some(existing) => existing.clone(),
        None => {
            let mut fresh = Session::new(
                draft.course_id.clone(),
                draft.weekday,
                draft.start.clone(),
                draft.end.clone(),
                draft.room.clone(),
            );
            if !draft.id.is_empty() {
                fresh.id = draft.id.clone();
            }
            fresh
        }
    };
    session.course_id = draft.course_id.clone();
    session.weekday = draft.weekday;
    session.start = draft.start.clone();
    session.end = draft.end.clone();
    session.room = draft.room.clone();
    db.save_session(session);
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> AnyElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let today = crate::model::today();
    let today_weekday = crate::model::iso_weekday(today);
    let pending = app.pending_session.clone();
    let now_minutes = crate::model::now_minutes();

    let this_add = this.clone();
    let action: AnyElement = Button::new("add-session")
        .primary()
        .compact()
        .icon(Icon::new(IconName::Plus))
        .label("新增安排")
        .on_click(move |_e, window, cx| {
            this_add.update(cx, |this, cx| {
                this.open_session_editor(SessionDraft::empty(), window, cx);
            });
        })
        .into_any_element();

    // MARK: 周课表网格

    // 7 列内容宽：56（时间槽列）+ 7×88（日列）+ 6×4（列间距）= 696。
    // 小于 696 时最右一列会在滚动容器内被裁切（DESIGN §4.3/§8）。
    let mut grid = div().flex().flex_col().gap_1().min_w(px(696.));

    // 表头
    let mut header = div()
        .w_full()
        .flex()
        .gap_1()
        .child(div().w(px(56.)).flex_shrink_0());
    for d in 1..=7u8 {
        let is_today = d == today_weekday;
        header = header.child(
            div()
                .w(px(88.))
                .flex_shrink_0()
                .flex()
                .flex_col()
                .items_center()
                .gap(px(0.))
                .py_2()
                .rounded_sm()
                .bg(if is_today {
                    theme.primary.with_alpha(0.14)
                } else {
                    Hsla {
                        a: 0.0,
                        ..theme.foreground
                    }
                })
                .child(
                    div()
                        .text_xs()
                        .text_color(if is_today {
                            theme.primary
                        } else {
                            theme.muted_foreground
                        })
                        .child(WEEKDAY_SHORT[(d - 1) as usize]),
                )
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(theme.foreground)
                        .child(crate::model::relative_day(crate::model::date_of(d))),
                ),
        );
    }
    grid = grid.child(header);

    if app.db.live_session_count() == 0 {
        let empty: AnyElement = div()
            .flex()
            .flex_col()
            .size_full()
            .p_5()
            .gap_4()
            .child(section_title(&theme, "计划表", Some(action)))
            .child(empty_state(
                &theme,
                IconName::CalendarDays,
                "还没有课表安排",
                Some("点击“新增安排”把课程排进每周计划"),
            ))
            .into_any_element();
        return div()
            .flex_1()
            .min_h_0()
            .child(empty)
            .when_some(pending, |d, e| d.child(overlay(&theme, e)))
            .into_any_element();
    }

    // 每天一列
    let mut body = div().w_full().flex().gap_1();
    for d in 1..=7u8 {
        let sessions: Vec<Session> = app.db.sessions_on(d).into_iter().cloned().collect();
        let is_today = d == today_weekday;
        let mut column = div()
            .w(px(88.))
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_1()
            .min_h(px(360.))
            .p_1()
            .rounded_sm()
            .border_1()
            .border_color(if is_today {
                theme.primary.with_alpha(0.5)
            } else {
                theme.border
            })
            .bg(if is_today {
                theme.primary.with_alpha(0.04)
            } else {
                panel(&theme)
            });

        if sessions.is_empty() {
            column = column.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("空闲"),
            );
        }

        for s in &sessions {
            let course = app.db.course(&s.course_id);
            let color = course
                .map(|c| palette_at_for(c.color, &theme))
                .unwrap_or(theme.primary);
            let name = course
                .map(|c| c.name.clone())
                .unwrap_or_else(|| "未知".into());
            let kind = course
                .map(|c| c.kind.short().to_string())
                .unwrap_or_default();
            let ongoing = now_minutes >= s.start_minutes() && now_minutes <= s.end_minutes();
            let id = s.id.clone();
            let _id_b = s.id.clone();
            let _this_del = this.clone();
            let this_edit = this.clone();

            column = column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(0.))
                    .p_2()
                    .rounded_sm()
                    .border_l_2()
                    .border_color(color)
                    .bg(tint(color, &theme))
                    .cursor_pointer()
                    .hover(|s| s.bg(theme.list_hover))
                    .id(eid("session-card", &s.id))
                    .on_click(move |_e, window, cx| {
                        this_edit.update(cx, |this, cx| {
                            let found = this.db.live_sessions().find(|x| x.id == id).cloned();
                            if let Some(found) = found {
                                let draft = SessionDraft::from(&found);
                                this.open_session_editor(draft, window, cx);
                            }
                        });
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_semibold()
                            .text_color(theme.foreground)
                            .child(name),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(format!("{} - {}", s.start, s.end)),
                    )
                    .when(!s.room.is_empty(), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(s.room.clone()),
                        )
                    })
                    .when(!kind.is_empty(), |d| {
                        d.child(div().text_xs().text_color(color).child(kind))
                    })
                    .when(ongoing, |d| {
                        d.child(
                            div()
                                .mt_1()
                                .flex()
                                .items_center()
                                .gap_1()
                                .text_xs()
                                .text_color(theme.success)
                                .child(div().size(px(5.)).rounded_full().bg(theme.success))
                                .child("进行中"),
                        )
                    }),
            );
        }

        // 添加按钮
        let this_add_cell = this.clone();
        column = column.child(
            div()
                .id(eid("add-cell", &d.to_string()))
                .h(px(24.))
                .flex()
                .items_center()
                .justify_center()
                .rounded_sm()
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.bg(theme.list_hover))
                .on_click(move |_e, window, cx| {
                    this_add_cell.update(cx, |this, cx| {
                        let mut draft = SessionDraft::empty();
                        draft.weekday = d;
                        this.open_session_editor(draft, window, cx);
                    });
                })
                .child(Icon::new(IconName::Plus).with_size(Size::XSmall)),
        );

        body = body.child(column);
    }
    grid = grid.child(body);

    // MARK: 右侧：本周概况 + 安排清单

    let mut by_day_list = div().flex().flex_col().gap_2();
    let mut sorted: Vec<&Session> = app.db.live_sessions().collect();
    sorted.sort_by_key(|s| (s.weekday, s.start_minutes()));
    for s in sorted.iter().take(12) {
        let course = app.db.course(&s.course_id);
        let color = course
            .map(|c| palette_at_for(c.color, &theme))
            .unwrap_or(theme.primary);
        let name = course
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "未知".into());
        let id = s.id.clone();
        let this_del = this.clone();

        by_day_list = by_day_list.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py(px(6.))
                .rounded_sm()
                .border_1()
                .border_color(theme.border)
                .child(div().size(px(6.)).flex_shrink_0().rounded_full().bg(color))
                .child(
                    // 名称单独一行，避免右侧操作按钮把文字挤没。
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap(px(0.))
                        .child(
                            div()
                                .text_sm()
                                .text_color(theme.foreground)
                                .whitespace_nowrap()
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(name),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .whitespace_nowrap()
                                .child(format!(
                                    "周{}  {} - {}",
                                    WEEKDAY_SHORT[(s.weekday - 1) as usize],
                                    s.start,
                                    s.end
                                )),
                        ),
                )
                .child(action_button(
                    eid("session-del", &s.id),
                    "删除",
                    move |_e, window, cx| {
                        confirm_delete_session(this_del.clone(), window, cx, id.clone());
                    },
                )),
        );
    }

    let school_minutes: i64 = app
        .db
        .live_sessions()
        .filter(|s| {
            app.db
                .course(&s.course_id)
                .map(|c| c.kind == crate::model::CourseKind::School)
                .unwrap_or(false)
        })
        .map(|s| (s.end_minutes() - s.start_minutes()).max(0) as i64)
        .sum();
    let tutoring_minutes = app.db.weekly_minutes() - school_minutes;

    let summary = card(
        &theme,
        "本周概况",
        None::<SharedString>,
        div()
            .flex()
            .flex_col()
            .gap_2()
            .child(summary_row(
                &theme,
                "校内课时",
                format!("{} 分钟", school_minutes),
                theme.info,
            ))
            .child(summary_row(
                &theme,
                "补习课时",
                format!("{} 分钟", tutoring_minutes),
                theme.warning,
            ))
            .child(summary_row(
                &theme,
                "每周合计",
                format!("{} 分钟", app.db.weekly_minutes()),
                theme.primary,
            ))
            .child(summary_row(
                &theme,
                "安排条数",
                format!("{} 条", app.db.live_session_count()),
                theme.foreground,
            )),
    );

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap_4()
        .child(section_title(&theme, "计划表", Some(action)))
        .child(
            div()
                .flex()
                .gap_4()
                .flex_1()
                .min_h_0()
                .child(div().flex_1().min_w_0().overflow_scrollbar().child(grid))
                .child(
                    div()
                        .w(px(260.))
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .gap_3()
                        .overflow_y_scrollbar()
                        .child(summary)
                        .child(card(&theme, "安排清单", None::<SharedString>, by_day_list)),
                ),
        )
        .when_some(pending, |d, e| d.child(overlay(&theme, e)))
        .into_any_element()
}

fn summary_row(theme: &Theme, label: &str, value: String, color: Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_2()
        .px_2()
        .py(px(6.))
        .rounded_sm()
        .bg(color.with_alpha(0.10))
        .child(
            div()
                .text_xs()
                .text_color(theme.foreground)
                .child(label.to_string()),
        )
        .child(
            div()
                .text_xs()
                .font_semibold()
                .text_color(color)
                .child(value),
        )
}

fn overlay(theme: &Theme, editor: Entity<SessionEditor>) -> impl IntoElement {
    div()
        .absolute()
        .inset_0()
        .bg(Hsla {
            a: 0.45,
            ..theme.foreground
        })
        .flex()
        .items_center()
        .justify_center()
        .p_6()
        .child(editor)
}

/// 删除确认对话框。
fn confirm_delete_session(owner: Entity<App>, window: &mut Window, cx: &mut gpui::App, id: String) {
    window.open_alert_dialog(cx, move |alert, _w, _cx| {
        let owner = owner.clone();
        let id = id.clone();
        alert
            .title("删除安排")
            .description("确定从课表中删除这条安排吗？")
            .ok_text("删除")
            .ok_variant(ButtonVariant::Danger)
            .on_ok(move |_e, window, cx| {
                owner.update(cx, |this, cx| {
                    this.db.delete_session(&id);
                    this.commit(window, cx);
                });
                true
            })
    });
}
