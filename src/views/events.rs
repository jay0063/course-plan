use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::WindowExt;
use gpui::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use chrono::Datelike;
use std::rc::Rc;

use crate::app::App;
use crate::model::{self, Database, EventItem, fmt_date_cn};
use crate::ui::form::field_row;
use crate::ui::widgets::{
    action_button, close_editor, eid, empty_state, palette_at, panel, section_title,
};

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

pub struct EventEditor {
    owner: Entity<App>,
    draft: EventDraft,
    title: Entity<InputState>,
    date: Entity<InputState>,
    start: Entity<InputState>,
    end: Entity<InputState>,
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
        let d = draft.date.clone();
        let s = draft.start.clone();
        let e = draft.end.clone();
        let l = draft.location.clone();

        let editor = Self {
            owner,
            draft: draft.clone(),
            title: cx.new(|cx| InputState::new(window, cx).placeholder("日程标题")),
            date: cx.new(|cx| InputState::new(window, cx).placeholder("2026-03-05")),
            start: cx.new(|cx| InputState::new(window, cx).placeholder("09:00")),
            end: cx.new(|cx| InputState::new(window, cx).placeholder("10:00")),
            location: cx.new(|cx| InputState::new(window, cx).placeholder("地点")),
            note: cx.new(|cx| TextareaState::new(window, cx).placeholder("备注")),
            on_save: Rc::new(on_save),
        };

        editor.title.update(cx, |x, cx| x.set_value(t, window, cx));
        editor.date.update(cx, |x, cx| x.set_value(d, window, cx));
        editor.start.update(cx, |x, cx| x.set_value(s, window, cx));
        editor.end.update(cx, |x, cx| x.set_value(e, window, cx));
        editor
            .location
            .update(cx, |x, cx| x.set_value(l, window, cx));

        cx.new(|_| editor)
    }

    fn snapshot(&self, cx: &gpui::App) -> EventDraft {
        EventDraft {
            id: self.draft.id.clone(),
            title: self.title.read(cx).value().to_string(),
            date: self.date.read(cx).value().to_string(),
            start: self.start.read(cx).value().to_string(),
            end: self.end.read(cx).value().to_string(),
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
        let this_close = this.clone();
        let date = self.date.clone();
        let start = self.start.clone();
        let end = self.end.clone();
        let location = self.location.clone();
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
                    .child("新建日程"),
            )
            .child(field_row("标题", Input::new(&title).h(px(32.)).w_full()))
            .child(field_row("日期", Input::new(&date).h(px(32.)).w_full()))
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
            .child(field_row("地点", Input::new(&location).h(px(32.)).w_full()))
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
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("日期格式：2026-03-05；时间格式：09:00"),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .pt_1()
                    .child(
                        Button::new("event-cancel")
                            .secondary()
                            .compact()
                            .label("取消")
                            .on_click(move |_e, _w, app| {
                                close_editor(this_close.read(app).owner.clone(), app);
                            }),
                    )
                    .child(
                        Button::new("event-save")
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

pub fn apply_draft(db: &mut Database, draft: &EventDraft) {
    let id = if draft.id.is_empty() {
        crate::model::new_id()
    } else {
        draft.id.clone()
    };
    match db.events.iter_mut().find(|e| e.id == id) {
        Some(item) => {
            item.title = draft.title.clone();
            item.date = draft.date.clone();
            item.start = draft.start.clone();
            item.end = draft.end.clone();
            item.location = draft.location.clone();
            item.note = draft.note.clone();
        }
        None => db.events.push(EventItem {
            id,
            title: draft.title.clone(),
            date: draft.date.clone(),
            start: draft.start.clone(),
            end: draft.end.clone(),
            location: draft.location.clone(),
            course_id: draft.course_id.clone(),
            note: draft.note.clone(),
            done: false,
        }),
    }
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let pending = app.pending_event.clone();
    let selected_day = app.selected_date;

    let this_add = this.clone();
    let action: AnyElement = Button::new("add-event")
        .primary()
        .compact()
        .icon(Icon::new(IconName::Plus))
        .label("新建日程")
        .on_click(move |_e, window, cx| {
            this_add.update(cx, |this, cx| {
                let mut draft = EventDraft::empty();
                draft.date = model::fmt_date(this.selected_date);
                this.open_event_editor(draft, window, cx);
            });
        })
        .into_any_element();

    // 日期选择条
    let mut days = div().flex().gap_1();
    for offset in -3i64..=3 {
        let date = model::today() + chrono::Duration::days(offset);
        let active = date == selected_day;
        let this = this.clone();
        days = days.child(
            div()
                .id(eid("day", &model::fmt_date(date)))
                .w(px(56.))
                .flex()
                .flex_col()
                .items_center()
                .gap(px(0.))
                .py_2()
                .rounded_md()
                .cursor_pointer()
                .border_1()
                .border_color(if active { theme.primary } else { theme.border })
                .bg(if active {
                    theme.primary.with_alpha(0.12)
                } else {
                    panel(&theme)
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
                        .text_xs()
                        .text_color(if active {
                            theme.primary
                        } else {
                            theme.muted_foreground
                        })
                        .child(model::relative_day(date)),
                )
                .child(
                    div()
                        .text_sm()
                        .font_semibold()
                        .text_color(theme.foreground)
                        .child(format!("{}/{}", date.month(), date.day())),
                ),
        );
    }

    // 事件列表
    let mut events: Vec<EventItem> = app
        .db
        .events_on(selected_day)
        .into_iter()
        .cloned()
        .collect();
    events.sort_by_key(|e| e.start.clone());

    let mut list = div().flex().flex_col().gap_2();
    if events.is_empty() {
        list = list.child(empty_state(
            &theme,
            IconName::Clock,
            "这一天还没有日程",
            Some("点击“新建日程”添加"),
        ));
    }
    for e in &events {
        let color = e
            .course_id
            .as_ref()
            .and_then(|id| app.db.course(id))
            .map(|c| palette_at(c.color))
            .unwrap_or(theme.info);
        let id = e.id.clone();
        let id_b = e.id.clone();
        let this_edit = this.clone();
        let this_del = this.clone();
        let title = e.title.clone();
        let location = e.location.clone();
        let note = e.note.clone();
        let time_text = format!("{} - {}", e.start, e.end);

        list = list.child(
            div()
                .flex()
                .items_start()
                .gap_3()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .bg(panel(&theme))
                .child(
                    div()
                        .w(px(4.))
                        .h(px(38.))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(color),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(theme.foreground)
                                .child(title),
                        )
                        .child(
                            div()
                                .flex()
                                .items_center()
                                .gap_2()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(time_text)
                                .when(!location.is_empty(), |d| d.child(format!("@ {location}"))),
                        )
                        .when(!note.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(note),
                            )
                        }),
                )
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(action_button(
                            eid("event-edit", &e.id),
                            "编辑",
                            move |_e, window, cx| {
                                this_edit.update(cx, |this, cx| {
                                    if let Some(found) = this.db.events.iter().find(|x| x.id == id)
                                    {
                                        let draft = EventDraft::from(found);
                                        this.open_event_editor(draft, window, cx);
                                    }
                                });
                            },
                        ))
                        .child(action_button(
                            eid("event-del", &e.id),
                            "删除",
                            move |_e, window, cx| {
                                let owner = this_del.clone();
                                confirm_delete_event(owner, window, cx, id_b.clone());
                            },
                        )),
                ),
        );
    }

    // 当天课程
    let weekday = model::iso_weekday(selected_day);
    let courses: Vec<String> = app
        .db
        .sessions_on(weekday)
        .into_iter()
        .map(|s| {
            let name = app.db.course_name(&s.course_id);
            format!("{} - {}  {}", s.start, s.end, name)
        })
        .collect();

    let course_card = div()
        .p_4()
        .rounded_lg()
        .border_1()
        .border_color(theme.border)
        .bg(panel(&theme))
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_semibold()
                .text_color(theme.foreground)
                .child("当天课程"),
        )
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(fmt_date_cn(selected_day)),
        )
        .when(courses.is_empty(), |d| {
            d.child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("这一天没有课程"),
            )
        });
    let mut course_card = course_card;
    for c in &courses {
        course_card = course_card.child(
            div()
                .px_2()
                .py(px(6.))
                .rounded_sm()
                .bg(theme.primary.with_alpha(0.10))
                .text_xs()
                .text_color(theme.foreground)
                .child(c.clone()),
        );
    }

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap_4()
        .child(section_title(&theme, "日程安排", Some(action)))
        .child(div().flex().gap_1().overflow_x_scrollbar().child(days))
        .child(
            div()
                .flex()
                .gap_4()
                .flex_1()
                .min_h_0()
                .child(div().flex_1().min_w_0().overflow_y_scrollbar().child(list))
                .child(
                    div()
                        .w(px(220.))
                        .flex_shrink_0()
                        .overflow_y_scrollbar()
                        .child(course_card),
                ),
        )
        .when_some(pending, |d, e| {
            d.child(
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
                    .child(e),
            )
        })
}

/// 删除确认对话框。
fn confirm_delete_event(owner: Entity<App>, window: &mut Window, cx: &mut gpui::App, id: String) {
    window.open_alert_dialog(cx, move |alert, _w, _cx| {
        let owner = owner.clone();
        let id = id.clone();
        alert
            .title("删除日程")
            .description("确定删除这条日程吗？")
            .ok_text("删除")
            .ok_variant(ButtonVariant::Danger)
            .on_ok(move |_e, window, cx| {
                owner.update(cx, |this, cx| {
                    this.db.events.retain(|x| x.id != id);
                    this.commit(window, cx);
                });
                true
            })
    });
}
