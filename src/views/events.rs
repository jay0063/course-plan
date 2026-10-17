//! 日程安排（DESIGN.md §4.4）。
//!
//! 顶部日期选择条（左右箭头 + 7 天快捷条 + 「今天」），下方当日日程列表
//! （8px 圆点 + 时间段 + 标题 + 地点 + 完成勾选），右侧保留「当天课程」。
//! 页面名 + 主 CTA 归顶栏（§3.1.4）。

use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::calendar::Date;
use gpui::component::date_picker::{DatePicker, DatePickerState};
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::component::time_field::{TimeField, TimeFieldState};
use gpui::*;

use chrono::{Datelike, NaiveDate};
use std::rc::Rc;

use crate::app::{App, Page};
use crate::model::{self, Database, EventItem, fmt_date_cn};
use crate::ui::components::button;
use crate::ui::components::card::{CardVariant, card_with_theme};
use crate::ui::components::check_circle::check_circle;
use crate::ui::components::empty_state::{EmptyVariant, empty_state};
use crate::ui::components::list_row::list_row;
use crate::ui::components::overlay::{dialog, overlay, section_title};
use crate::ui::components::panel as panel_helper;
use crate::ui::form::field_row;
use crate::ui::palette::palette_at_for;
use crate::ui::tokens::text;
use crate::ui::tokens::{
    INPUT_H, RADIUS_FULL, RADIUS_MD, SPACE_2XL, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XL, SPACE_XS,
};
use crate::ui::widgets::{close_editor, eid};

use crate::views::courses::{
    DeleteKind, delete_confirm_overlay, discard_stale_delete, overlay_click_guard, pending_delete,
    request_delete,
};

/// `list_row` 的「无点击」占位类型（`App` 指 gpui 的 App，不是本应用的 App）。
type RowClick = fn(&ClickEvent, &mut Window, &mut gpui::App);

/// 右侧「当天课程」栏宽度。
const RAIL_W: Pixels = px(220.);
/// 日期间隔条单元格宽度。
const DAY_CELL_W: Pixels = px(56.);

#[derive(Clone)]
pub struct EventDraft {
    pub id: String,
    pub title: String,
    pub date: String,
    pub start: String,
    pub end: String,
    pub location: String,
    pub course_id: Option<String>,
    pub note: String,
}

impl EventDraft {
    pub fn empty() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            date: model::fmt_date(model::today()),
            start: "09:00".into(),
            end: "10:00".into(),
            location: String::new(),
            course_id: None,
            note: String::new(),
        }
    }

    pub fn from(item: &EventItem) -> Self {
        Self {
            id: item.id.clone(),
            title: item.title.clone(),
            date: item.date.clone(),
            start: item.start.clone(),
            end: item.end.clone(),
            location: item.location.clone(),
            course_id: item.course_id.clone(),
            note: item.note.clone(),
        }
    }
}

type SaveCb = Rc<dyn Fn(EventDraft, &mut Window, &mut gpui::App)>;

fn parse_date_or_today(text: &str) -> NaiveDate {
    NaiveDate::parse_from_str(text.trim(), "%Y-%m-%d").unwrap_or_else(|_| model::today())
}

fn time_or(text: &str, fallback: (u32, u32)) -> chrono::NaiveTime {
    model::parse_time(text)
        .unwrap_or_else(|| chrono::NaiveTime::from_hms_opt(fallback.0, fallback.1, 0).unwrap())
}

pub struct EventEditor {
    owner: Entity<App>,
    draft: EventDraft,
    title: Entity<InputState>,
    date: Entity<DatePickerState>,
    start: Entity<TimeFieldState>,
    end: Entity<TimeFieldState>,
    location: Entity<InputState>,
    note: Entity<TextareaState>,
    on_save: SaveCb,
}

impl EventEditor {
    pub fn new(
        draft: &EventDraft,
        window: &mut Window,
        cx: &mut Context<App>,
        on_save: impl Fn(EventDraft, &mut Window, &mut gpui::App) + 'static,
    ) -> Entity<Self> {
        let owner = cx.entity();
        let t = draft.title.clone();
        let l = draft.location.clone();
        let initial_date = parse_date_or_today(&draft.date);
        let start_time = time_or(&draft.start, (9, 0));
        let end_time = time_or(&draft.end, (10, 0));

        let date = cx.new(|cx| {
            DatePickerState::new(window, cx)
                .date_format("%Y-%m-%d")
                .first_day_of_week(chrono::Weekday::Mon)
        });
        date.update(cx, |state, cx| {
            state.set_date(initial_date, window, cx);
        });

        let editor = Self {
            owner,
            draft: draft.clone(),
            title: cx.new(|cx| InputState::new(window, cx).placeholder("日程标题")),
            date,
            start: cx.new(|cx| TimeFieldState::new(window, cx)),
            end: cx.new(|cx| TimeFieldState::new(window, cx)),
            location: cx.new(|cx| InputState::new(window, cx).placeholder("地点")),
            note: cx.new(|cx| TextareaState::new(window, cx).placeholder("备注")),
            on_save: Rc::new(on_save),
        };

        editor.title.update(cx, |x, cx| x.set_value(t, window, cx));
        editor
            .start
            .update(cx, |s, cx| s.set_time(start_time, window, cx));
        editor
            .end
            .update(cx, |s, cx| s.set_time(end_time, window, cx));
        editor
            .location
            .update(cx, |x, cx| x.set_value(l, window, cx));

        cx.new(|_| editor)
    }

    fn snapshot(&self, cx: &gpui::App) -> EventDraft {
        let date = match self.date.read(cx).date() {
            Date::Single(Some(date)) => model::fmt_date(date),
            // 未选择日期时保留原值，避免把日程日期清空。
            _ => self.draft.date.clone(),
        };
        EventDraft {
            id: self.draft.id.clone(),
            title: self.title.read(cx).value().to_string(),
            date,
            start: model::format_time(self.start.read(cx).time()),
            end: model::format_time(self.end.read(cx).time()),
            location: self.location.read(cx).value().to_string(),
            course_id: self.draft.course_id.clone(),
            note: self.note.read(cx).value().to_string(),
        }
    }
}

impl Render for EventEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();
        let title = self.title.clone();
        let date = self.date.clone();
        let start = self.start.clone();
        let end = self.end.clone();
        let location = self.location.clone();
        let note = self.note.clone();
        let heading = if self.draft.id.is_empty() {
            "新建日程"
        } else {
            "编辑日程"
        };

        // 日期用 gpui-kit 原生 DatePicker（自带日历下拉），时间用 TimeField（§3.15）。
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
            .child(field_row("标题", Input::new(&title).h(INPUT_H).w_full()))
            .child(field_row(
                "日期",
                DatePicker::new(&date)
                    .with_size(Size::Medium)
                    .h(INPUT_H)
                    .w_full(),
            ))
            .child(time_input("开始", &start))
            .child(time_input("结束", &end))
            .child(field_row("地点", Input::new(&location).h(INPUT_H).w_full()))
            .child(
                div()
                    .w_full()
                    .rounded(RADIUS_MD)
                    .border_1()
                    .border_color(theme.border)
                    .child(Textarea::new(&note).w_full()),
            );

        let cancel_this = this.clone();
        let save_this = this.clone();
        let footer = div()
            .flex()
            .gap(SPACE_SM)
            .child(button::secondary(
                "event-cancel",
                "取消",
                move |_e, _w, cx| {
                    close_editor(cancel_this.read(cx).owner.clone(), cx);
                },
            ))
            .child(button::primary(
                "event-save",
                "保存",
                move |_e, window, cx| {
                    let (draft, cb) = {
                        let guard = save_this.read(cx);
                        (guard.snapshot(cx), guard.on_save.clone())
                    };
                    cb(draft, window, cx);
                },
            ));

        dialog(&theme, heading, body, Some(footer)).into_any_element()
    }
}

pub fn apply_draft(db: &mut Database, draft: &EventDraft) {
    let mut event = match db.events.iter().find(|e| e.id == draft.id) {
        Some(existing) => existing.clone(),
        None => {
            let mut fresh = EventItem::new(draft.title.clone(), draft.date.clone());
            if !draft.id.is_empty() {
                fresh.id = draft.id.clone();
            }
            fresh
        }
    };
    event.title = draft.title.clone();
    event.date = draft.date.clone();
    event.start = draft.start.clone();
    event.end = draft.end.clone();
    event.location = draft.location.clone();
    event.note = draft.note.clone();
    event.course_id = draft.course_id.clone();
    db.save_event(event);
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let pending = app.pending_event.clone();
    let selected_day = app.selected_date;

    discard_stale_delete(cx, Page::Events);
    let deleting = pending_delete(cx);

    // MARK: 日期选择条（箭头 + 7 天快捷条 + 今天）

    let prev_this = this.clone();
    let prev = button::icon_only(
        "events-prev-day",
        IconName::ChevronLeft,
        "前一天",
        move |_e, _w, cx| {
            prev_this.update(cx, |app, cx| {
                app.selected_date -= chrono::Duration::days(1);
                cx.notify();
            });
        },
    );
    let next_this = this.clone();
    let next = button::icon_only(
        "events-next-day",
        IconName::ChevronRight,
        "后一天",
        move |_e, _w, cx| {
            next_this.update(cx, |app, cx| {
                app.selected_date += chrono::Duration::days(1);
                cx.notify();
            });
        },
    );
    let today_this = this.clone();
    let today_btn = button::ghost("events-today", "今天", move |_e, _w, cx| {
        today_this.update(cx, |app, cx| {
            app.selected_date = model::today();
            cx.notify();
        });
    });

    let mut strip = div().flex().gap(SPACE_XS);
    for offset in -3i64..=3 {
        let date = selected_day + chrono::Duration::days(offset);
        let active = date == selected_day;
        let this = this.clone();
        strip = strip.child(
            div()
                .id(eid("day", &model::fmt_date(date)))
                .w(DAY_CELL_W)
                .flex_shrink_0()
                .flex()
                .flex_col()
                .items_center()
                .py(SPACE_SM)
                .rounded(RADIUS_MD)
                .cursor_pointer()
                .border_1()
                .border_color(if active { theme.primary } else { theme.border })
                .bg(if active {
                    theme.primary.with_alpha(0.12)
                } else {
                    panel_helper::panel(&theme)
                })
                .hover(|s| s.border_color(theme.primary_hover))
                .on_click(move |_e, _w, cx| {
                    this.update(cx, |this, cx| {
                        this.selected_date = date;
                        cx.notify();
                    });
                })
                .child(
                    div()
                        .text_size(text::CAPTION)
                        .text_color(if active {
                            theme.primary
                        } else {
                            theme.muted_foreground
                        })
                        .child(model::relative_day(date)),
                )
                .child(
                    div()
                        .text_size(text::BODY_SM)
                        .font_weight(FontWeight(600.0))
                        .text_color(theme.foreground)
                        .child(format!("{}/{}", date.month(), date.day())),
                ),
        );
    }

    let date_nav = div()
        .flex()
        .items_center()
        .gap(SPACE_SM)
        .child(prev)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .gap(SPACE_XS)
                .overflow_x_scrollbar()
                .child(strip),
        )
        .child(next)
        .child(today_btn);

    // MARK: 当日日程

    let mut events: Vec<EventItem> = app
        .db
        .events_on(selected_day)
        .into_iter()
        .cloned()
        .collect();
    events.sort_by_key(|e| e.start.clone());

    let mut list = div().flex().flex_col().gap(SPACE_MD);
    if events.is_empty() {
        list = list.child(empty_state(
            &theme,
            EmptyVariant::Illustrated,
            IconName::Clock,
            "这一天还没有日程",
            Some("点右上角「快速添加」新建日程"),
            None::<AnyElement>,
        ));
    }

    for e in &events {
        let color = e
            .course_id
            .as_ref()
            .and_then(|id| app.db.course(id))
            .map(|c| palette_at_for(c.color, &theme))
            .unwrap_or(theme.primary);
        let done = e.done;
        let title = e.title.clone();
        let location = e.location.clone();
        let note = e.note.clone();
        let time_text = format!("{} - {}", e.start, e.end);

        let id_row = e.id.clone();
        let id_toggle = e.id.clone();
        let id_edit = e.id.clone();
        let id_del = e.id.clone();
        let name_del = e.title.clone();
        let this_row = this.clone();
        let this_toggle = this.clone();
        let this_edit = this.clone();
        let this_del = this.clone();

        let detail = div()
            .id(eid("event-row", &e.id))
            .flex_1()
            .min_w_0()
            .flex()
            .items_center()
            .gap(SPACE_MD)
            .cursor_pointer()
            .on_click(move |_e, window, cx| {
                this_row.update(cx, |this, cx| {
                    let found = this.db.live_events().find(|x| x.id == id_row).cloned();
                    if let Some(found) = found {
                        let draft = EventDraft::from(&found);
                        this.open_event_editor(draft, window, cx);
                    }
                });
            })
            .child(
                div()
                    .size(px(8.)) // 一次性像素：8px 圆点（§4.4）
                    .flex_shrink_0()
                    .rounded(RADIUS_FULL)
                    .bg(color),
            )
            .child(
                div()
                    .w(px(96.)) // 一次性像素：时间段列宽，保证标题对齐
                    .flex_shrink_0()
                    .text_size(text::CAPTION)
                    .text_color(theme.muted_foreground)
                    .child(time_text),
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
                            .text_size(text::BODY_EMPH)
                            .font_weight(FontWeight(600.0))
                            .text_color(theme.foreground)
                            .when(done, |d| {
                                d.line_through().text_color(theme.muted_foreground)
                            })
                            .child(title),
                    )
                    .when(!location.is_empty(), |d| {
                        d.child(
                            div()
                                .text_size(text::CAPTION)
                                .text_color(theme.muted_foreground)
                                .child(format!("@ {location}")),
                        )
                    })
                    .when(!note.is_empty(), |d| {
                        d.child(
                            div()
                                .text_size(text::CAPTION)
                                .text_color(theme.muted_foreground)
                                .child(note.clone()),
                        )
                    }),
            );

        list = list.child(card_with_theme(
            &theme,
            CardVariant::Default,
            None::<SharedString>,
            None::<SharedString>,
            div()
                .flex()
                .items_center()
                .gap(SPACE_MD)
                .child(detail)
                .child(
                    div()
                        .id(eid("event-done", &e.id))
                        .flex_shrink_0()
                        .cursor_pointer()
                        .on_click(move |_e, window, cx| {
                            this_toggle.update(cx, |this, cx| {
                                let done_now = this
                                    .db
                                    .live_events()
                                    .find(|x| x.id == id_toggle)
                                    .map(|x| x.done)
                                    .unwrap_or(false);
                                this.db.set_event_done(&id_toggle, !done_now);
                                this.commit(window, cx);
                            });
                        })
                        .child(check_circle(&theme, done, px(20.))),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap(SPACE_XS)
                        .flex_shrink_0()
                        .child(button::icon_only(
                            eid("event-edit", &e.id),
                            IconName::Pencil,
                            "编辑日程",
                            move |_e, window, cx| {
                                this_edit.update(cx, |this, cx| {
                                    let found =
                                        this.db.live_events().find(|x| x.id == id_edit).cloned();
                                    if let Some(found) = found {
                                        let draft = EventDraft::from(&found);
                                        this.open_event_editor(draft, window, cx);
                                    }
                                });
                            },
                        ))
                        .child(button::icon_only(
                            eid("event-del", &e.id),
                            IconName::Trash,
                            "删除日程",
                            move |_e, _window, cx| {
                                this_del.update(cx, |_this, cx| {
                                    request_delete(
                                        cx,
                                        Page::Events,
                                        DeleteKind::Event,
                                        id_del.clone(),
                                        name_del.clone(),
                                    );
                                });
                            },
                        )),
                ),
        ));
    }

    // MARK: 右侧「当天课程」（保留原有信息密度）

    let weekday = model::iso_weekday(selected_day);
    let sessions = app.db.sessions_on(weekday);
    let mut course_rows = div().flex().flex_col().gap(SPACE_XS);
    if sessions.is_empty() {
        course_rows = course_rows.child(
            div()
                .py(SPACE_SM)
                .text_size(text::CAPTION)
                .text_color(theme.muted_foreground)
                .child("这一天没有课程"),
        );
    }
    for s in &sessions {
        let color = app
            .db
            .course(&s.course_id)
            .map(|c| palette_at_for(c.color, &theme))
            .unwrap_or(theme.primary);
        course_rows = course_rows.child(list_row(
            &theme,
            eid("event-course", &s.id),
            None::<RowClick>,
            Some(div().size(px(8.)).rounded(RADIUS_FULL).bg(color)),
            app.db.course_name(&s.course_id),
            Some(format!("{} - {}", s.start, s.end)),
            None::<AnyElement>,
        ));
    }

    let count: AnyElement = div()
        .text_size(text::LABEL_SM)
        .text_color(theme.muted_foreground)
        .child(format!("{} 条", events.len()))
        .into_any_element();

    let overlay_open = pending.is_some() || deleting.is_some();

    div()
        .flex()
        .flex_col()
        .size_full()
        .p(SPACE_2XL)
        .gap(SPACE_XL)
        .child(date_nav)
        .child(section_title(&theme, "当天安排", Some(count)))
        .child(
            div()
                .flex()
                .gap(SPACE_LG)
                .flex_1()
                .min_h_0()
                .child(div().flex_1().min_w_0().overflow_y_scrollbar().child(list))
                .child(
                    div()
                        .w(RAIL_W)
                        .flex_shrink_0()
                        .overflow_y_scrollbar()
                        .child(card_with_theme(
                            &theme,
                            CardVariant::Default,
                            Some("当天课程"),
                            Some(fmt_date_cn(selected_day)),
                            course_rows,
                        )),
                ),
        )
        .when(overlay_open, |d| d.child(overlay_click_guard()))
        .when_some(pending, |d, e| d.child(overlay(&theme, e)))
        .when_some(deleting, |d, req| {
            d.child(delete_confirm_overlay(&theme, &req, this.clone()))
        })
}
