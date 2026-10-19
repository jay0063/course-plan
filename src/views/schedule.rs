//! 计划表（DESIGN.md §4.3）。
//!
//! 7 列固定宽度周表（88px 日列 + 56px 时间槽列 + 4px 间距 = 最小 696），
//! 今天列高亮，右侧 260px 概况 + 安排清单。页面名 + 主 CTA 归顶栏（§3.1.4）。

use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::input::{Input, InputState};
use gpui::component::scroll::ScrollableElement;
use gpui::component::time_field::{TimeField, TimeFieldState};
use gpui::*;

use std::rc::Rc;

use crate::app::{App, Page};
use crate::model::{self, Database, Session, WEEKDAY_LABELS, WEEKDAY_SHORT};
use crate::ui::components::button;
use crate::ui::components::card::{CardVariant, card_with_theme};
use crate::ui::components::empty_state::{EmptyVariant, empty_state};
use crate::ui::components::list_row::list_row;
use crate::ui::components::overlay::{dialog, overlay, section_title};
use crate::ui::components::panel as panel_helper;
use crate::ui::components::tag::tag;
use crate::ui::form::field_row;
use crate::ui::palette::{palette_at_for, tint};
use crate::ui::select::{Select, SelectOption};
use crate::ui::tokens::text;
use crate::ui::tokens::{
    INPUT_H, RADIUS_FULL, RADIUS_SM, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XL, SPACE_XS,
};
use crate::ui::widgets::{close_editor, eid};

use crate::views::courses::{
    DeleteKind, delete_confirm_overlay, discard_stale_delete, pending_delete, request_delete,
};

/// `list_row` 的「无点击」占位类型（`App` 指 gpui 的 App，不是本应用的 App）。
type RowClick = fn(&ClickEvent, &mut Window, &mut gpui::App);

/// 7 列网格的最小宽度（56 + 7×88 + 6×4 = 696，DESIGN §4.3）。
const GRID_MIN_W: Pixels = px(696.);
/// 日列固定宽度。
const DAY_COL_W: Pixels = px(88.);
/// 时间槽列宽度。
const TIME_COL_W: Pixels = px(56.);
/// 侧栏（本周概况 + 安排清单）宽度。
const RAIL_W: Pixels = px(260.);

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

fn time_or(text: &str, fallback: (u32, u32)) -> chrono::NaiveTime {
    model::parse_time(text)
        .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(fallback.0, fallback.1, 0).unwrap())
}

type SaveCb = Rc<dyn Fn(SessionDraft, &mut Window, &mut gpui::App)>;

pub struct SessionEditor {
    owner: Entity<App>,
    draft: SessionDraft,
    courses: Vec<(String, String)>,
    course: Entity<Select>,
    weekday: Entity<Select>,
    start: Entity<TimeFieldState>,
    end: Entity<TimeFieldState>,
    room: Entity<InputState>,
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

        let start_time = time_or(&draft.start, (8, 0));
        let end_time = time_or(&draft.end, (8, 45));
        let room_text = draft.room.clone();

        let editor = Self {
            owner,
            draft: draft.clone(),
            courses,
            course: cx.new(|_| {
                Select::new("session-course")
                    .placeholder("选择课程")
                    .options(options)
                    .selected(selected)
            }),
            weekday: cx.new(|_| {
                Select::new("session-weekday")
                    .options(day_options)
                    .selected(Some(draft.weekday.saturating_sub(1) as usize))
            }),
            start: cx.new(|cx| TimeFieldState::new(window, cx)),
            end: cx.new(|cx| TimeFieldState::new(window, cx)),
            room: cx.new(|cx| InputState::new(window, cx).placeholder("教室 / 地点")),
            on_save: Rc::new(on_save),
        };

        editor
            .start
            .update(cx, |s, cx| s.set_time(start_time, window, cx));
        editor
            .end
            .update(cx, |s, cx| s.set_time(end_time, window, cx));
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
            start: model::format_time(self.start.read(cx).time()),
            end: model::format_time(self.end.read(cx).time()),
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
        let end = self.end.clone();
        let room = self.room.clone();
        let title = if self.draft.id.is_empty() {
            "新建安排"
        } else {
            "编辑安排"
        };

        // 开始 / 结束用 gpui-kit 原生 TimeField（§3.15），不再用手写文本输入。
        let time_input = |label: &'static str, state: &Entity<TimeFieldState>| {
            field_row(
                label,
                TimeField::new(state)
                    .with_size(Size::Medium)
                    .h(INPUT_H)
                    .w_full(),
            )
        };

        let body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(SPACE_MD)
            .child(field_row("所属课程", div().flex().child(course)))
            .child(field_row("星期", div().flex().child(weekday)))
            .child(time_input("开始", &start))
            .child(time_input("结束", &end))
            .child(field_row("教室", Input::new(&room).h(INPUT_H).w_full()));

        let cancel_this = this.clone();
        let save_this = this.clone();
        let footer = div()
            .flex()
            .gap(SPACE_SM)
            .child(button::secondary(
                "session-cancel",
                "取消",
                move |_e, _w, cx| {
                    close_editor(cancel_this.read(cx).owner.clone(), cx);
                },
            ))
            .child(button::primary(
                "session-save",
                "保存",
                move |_e, window, cx| {
                    let (draft, cb) = {
                        let guard = save_this.read(cx);
                        (guard.snapshot(cx), guard.on_save.clone())
                    };
                    cb(draft, window, cx);
                },
            ));

        dialog(&theme, title, body, Some(footer)).into_any_element()
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
    let today_weekday = model::iso_weekday(model::today());
    let pending = app.pending_session.clone();
    let now_minutes = model::now_minutes();

    discard_stale_delete(cx, Page::Schedule);
    let deleting = pending_delete(cx);

    // MARK: 周课表网格（7 列固定宽度 + 水平滚动）

    let mut grid = div().flex().flex_col().gap(SPACE_XS).min_w(GRID_MIN_W);

    // 表头
    let mut header = div()
        .w_full()
        .flex()
        .gap(SPACE_XS)
        .child(div().w(TIME_COL_W).flex_shrink_0());
    for d in 1..=7u8 {
        let is_today = d == today_weekday;
        header = header.child(
            div()
                .w(DAY_COL_W)
                .flex_shrink_0()
                .flex()
                .flex_col()
                .items_center()
                .py(SPACE_SM)
                .rounded(RADIUS_SM)
                .bg(if is_today {
                    theme.primary.with_alpha(0.14)
                } else {
                    theme.foreground.with_alpha(0.0)
                })
                .child(
                    div()
                        .text_size(text::CAPTION)
                        .text_color(if is_today {
                            theme.primary
                        } else {
                            theme.muted_foreground
                        })
                        .child(WEEKDAY_SHORT[(d - 1) as usize]),
                )
                .child(
                    div()
                        .text_size(text::BODY_SM)
                        .font_weight(FontWeight(600.0))
                        .text_color(theme.foreground)
                        .child(model::relative_day(model::date_of(d))),
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
            .gap(SPACE_XL)
            .child(section_title(&theme, "本周安排", None::<AnyElement>))
            .child(empty_state(
                &theme,
                EmptyVariant::Illustrated,
                IconName::CalendarDays,
                "本周没有额外安排",
                Some("点右上角「快速添加」排进每周计划"),
                None::<AnyElement>,
            ))
            .into_any_element();
        // 根节点必须 `size_full()`：`page_enter` 是确定高度的父层，若这里用
        // `flex_1().min_h_0()` 则高度按内容（auto）解析，`absolute().inset_0()` 的
        // 遮罩会塌缩到近乎 0 高，对话框以这个零高盒居中 → 整个表单被推到视口上方，
        // 「选择课程」落在 y<0，点不到也存不了。零安排是新装后的第一个状态，
        // 因此必须保证这里的根节点撑满内容区。
        return div()
            .flex()
            .flex_col()
            .relative()
            .size_full()
            .child(empty)
            .when_some(pending, |d, e| d.child(overlay(&theme, e)))
            .when_some(deleting, |d, req| {
                d.child(delete_confirm_overlay(&theme, &req, this.clone()))
            })
            .into_any_element();
    }

    // 每天一列（与表头一样先放 56px 时间槽列，保证日列与表头对齐、网格宽 696）
    let mut body = div()
        .w_full()
        .flex()
        .gap(SPACE_XS)
        .child(div().w(TIME_COL_W).flex_shrink_0());
    for d in 1..=7u8 {
        let sessions: Vec<Session> = app.db.sessions_on(d).into_iter().cloned().collect();
        let is_today = d == today_weekday;
        let group = format!("day-col:{d}");

        let mut column = div()
            .group(group.clone())
            .w(DAY_COL_W)
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap(SPACE_XS)
            .min_h(px(360.)) // 一次性像素：网格最小高度，与 DESIGN §4.3 一致
            .p(SPACE_XS)
            .rounded(RADIUS_SM)
            .border(if is_today { px(1.5) } else { px(1.) })
            .border_color(if is_today {
                theme.primary
            } else {
                theme.border
            })
            .bg(if is_today {
                theme.primary.with_alpha(0.04)
            } else {
                panel_helper::panel(&theme)
            });

        if sessions.is_empty() {
            column = column.child(
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_size(text::CAPTION)
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
            let this_edit = this.clone();

            column = column.child(
                div()
                    .flex()
                    .items_stretch()
                    .gap(SPACE_XS)
                    .p(SPACE_SM)
                    .rounded(RADIUS_SM)
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
                            .w(px(3.)) // 一次性像素：课程色条
                            .flex_shrink_0()
                            .rounded(RADIUS_FULL)
                            .bg(color),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(SPACE_XS)
                            .child(
                                div()
                                    .text_size(text::CAPTION)
                                    .font_weight(FontWeight(600.0))
                                    .text_color(theme.foreground)
                                    .child(name),
                            )
                            .child(
                                div()
                                    .text_size(text::CAPTION)
                                    .text_color(theme.muted_foreground)
                                    .child(format!("{} - {}", s.start, s.end)),
                            )
                            .when(!s.room.is_empty(), |d| {
                                d.child(
                                    div()
                                        .text_size(text::CAPTION)
                                        .text_color(theme.muted_foreground)
                                        .child(s.room.clone()),
                                )
                            })
                            .when(!kind.is_empty(), |d| {
                                d.child(
                                    div().text_size(text::CAPTION).text_color(color).child(kind),
                                )
                            })
                            .when(ongoing, |d| {
                                d.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap(SPACE_XS)
                                        .text_size(text::CAPTION)
                                        .text_color(theme.success)
                                        .child(
                                            div()
                                                .size(px(5.))
                                                .rounded(RADIUS_FULL)
                                                .bg(theme.success),
                                        )
                                        .child("进行中"),
                                )
                            }),
                    ),
            );
        }

        // 列底「+」：hover 整列变 primary（§4.3）
        let this_add_cell = this.clone();
        let hover_group = group.clone();
        column = column.child(
            div()
                .id(eid("add-cell", &d.to_string()))
                .h(px(24.)) // 一次性像素：紧凑加号行
                .flex()
                .items_center()
                .justify_center()
                .rounded(RADIUS_SM)
                .cursor_pointer()
                .text_color(theme.muted_foreground)
                .hover(|s| s.bg(theme.list_hover).text_color(theme.primary))
                .group_hover(hover_group, |s| s.text_color(theme.primary))
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

    let mut by_day_list = div().flex().flex_col();
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

        by_day_list = by_day_list.child(list_row(
            &theme,
            eid("session-list", &s.id),
            None::<RowClick>,
            Some(div().size(px(8.)).rounded(RADIUS_FULL).bg(color)),
            name,
            Some(format!(
                "周{}  {} - {}",
                WEEKDAY_SHORT[(s.weekday - 1) as usize],
                s.start,
                s.end
            )),
            Some(button::icon_only(
                eid("session-del", &s.id),
                IconName::Trash,
                "删除安排",
                move |_e, _window, cx| {
                    this_del.update(cx, |_this, cx| {
                        request_delete(
                            cx,
                            Page::Schedule,
                            DeleteKind::Session,
                            id.clone(),
                            String::new(),
                        );
                    });
                },
            )),
        ));
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

    let summary = card_with_theme(
        &theme,
        CardVariant::Default,
        Some("本周概况"),
        None::<SharedString>,
        div()
            .flex()
            .flex_col()
            .gap(SPACE_XS)
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
                theme.success,
            )),
    );

    let count: AnyElement = div()
        .text_size(text::LABEL_SM)
        .text_color(theme.muted_foreground)
        .child(format!("共 {} 条", app.db.live_session_count()))
        .into_any_element();

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap(SPACE_XL)
        .child(section_title(&theme, "本周安排", Some(count)))
        .child(
            div()
                .flex()
                .gap(SPACE_LG)
                .flex_1()
                .min_h_0()
                .child(div().flex_1().min_w_0().overflow_scrollbar().child(grid))
                .child(
                    div()
                        .w(RAIL_W)
                        .flex_shrink_0()
                        .flex()
                        .flex_col()
                        .gap(SPACE_MD)
                        .overflow_y_scrollbar()
                        .child(summary)
                        .child(card_with_theme(
                            &theme,
                            CardVariant::Default,
                            Some("安排清单"),
                            None::<SharedString>,
                            by_day_list,
                        )),
                ),
        )
        .when_some(pending, |d, e| d.child(overlay(&theme, e)))
        .when_some(deleting, |d, req| {
            d.child(delete_confirm_overlay(&theme, &req, this.clone()))
        })
        .into_any_element()
}

/// 概况行：§3.6 `tag` 色块 + 数值（颜色块承担语义色）。
fn summary_row(theme: &Theme, label: &str, value: String, accent: Hsla) -> impl IntoElement {
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(SPACE_SM)
        .child(tag(theme, label, accent))
        .child(
            div()
                .text_size(text::LABEL_SM)
                .font_weight(FontWeight(600.0))
                .text_color(theme.foreground)
                .child(value),
        )
}
