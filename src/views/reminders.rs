use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::WindowExt;
use gpui::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use std::rc::Rc;

use crate::app::{App, is_due};
use crate::model::{self, Database, Reminder, Repeat, parse_time};
use crate::ui::form::field_row;
use crate::ui::select::{Select, SelectOption};
use crate::ui::widgets::{
    action_button, check_circle, chip, close_editor, eid, empty_state, panel, section_title,
};

#[derive(Clone)]
pub struct ReminderDraft {
    pub id: String,
    pub title: String,
    pub content: String,
    pub due_date: String,
    pub due_time: String,
    pub repeat: Repeat,
}

impl ReminderDraft {
    pub fn empty() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            content: String::new(),
            due_date: model::fmt_date(model::today()),
            due_time: "20:00".into(),
            repeat: Repeat::Once,
        }
    }

    pub fn from(r: &Reminder) -> Self {
        let date = r.due_at.get(..10).unwrap_or("").to_string();
        let time = if r.due_at.len() >= 16 {
            r.due_at[11..16].to_string()
        } else {
            "20:00".to_string()
        };
        Self {
            id: r.id.clone(),
            title: r.title.clone(),
            content: r.content.clone(),
            due_date: date,
            due_time: time,
            repeat: r.repeat,
        }
    }
}

type SaveCb = Rc<dyn Fn(ReminderDraft, &mut Window, &mut gpui::App)>;

pub struct ReminderEditor {
    owner: Entity<App>,
    draft: ReminderDraft,
    title: Entity<InputState>,
    content: Entity<TextareaState>,
    date: Entity<InputState>,
    time: Entity<InputState>,
    repeat: Entity<Select>,
    on_save: SaveCb,
}

impl ReminderEditor {
    pub fn new(
        draft: &ReminderDraft,
        window: &mut Window,
        cx: &mut Context<App>,
        on_save: impl Fn(ReminderDraft, &mut Window, &mut gpui::App) + 'static,
    ) -> Entity<Self> {
        let owner = cx.entity();
        let t = draft.title.clone();
        let c = draft.content.clone();
        let d = draft.due_date.clone();
        let tm = draft.due_time.clone();
        let repeat_index = Some(match draft.repeat {
            Repeat::Once => 0,
            Repeat::Daily => 1,
            Repeat::Weekly => 2,
        });

        let editor = Self {
            owner,
            draft: draft.clone(),
            title: cx
                .new(|cx| InputState::new(window, cx).placeholder("提醒内容，如：提交数学作业")),
            content: cx.new(|cx| TextareaState::new(window, cx).placeholder("补充说明")),
            date: cx.new(|cx| InputState::new(window, cx).placeholder("2026-03-05")),
            time: cx.new(|cx| InputState::new(window, cx).placeholder("20:00")),
            repeat: cx.new(|_| {
                Select::new("reminder-repeat")
                    .label("重复方式")
                    .options(vec![
                        SelectOption::new("仅一次"),
                        SelectOption::new("每天"),
                        SelectOption::new("每周"),
                    ])
                    .selected(repeat_index)
            }),
            on_save: Rc::new(on_save),
        };

        editor.title.update(cx, |x, cx| x.set_value(t, window, cx));
        editor.date.update(cx, |x, cx| x.set_value(d, window, cx));
        editor.time.update(cx, |x, cx| x.set_value(tm, window, cx));
        editor.content.update(cx, |x, cx| {
            x.set_auto_grow(2, 4, cx);
            x.set_value(c, window, cx);
        });

        cx.new(|_| editor)
    }

    fn snapshot(&self, cx: &gpui::App) -> ReminderDraft {
        ReminderDraft {
            id: self.draft.id.clone(),
            title: self.title.read(cx).value().to_string(),
            content: self.content.read(cx).value().to_string(),
            due_date: self.date.read(cx).value().to_string(),
            due_time: self.time.read(cx).value().to_string(),
            repeat: match self.repeat.read(cx).selected_index() {
                Some(1) => Repeat::Daily,
                Some(2) => Repeat::Weekly,
                _ => Repeat::Once,
            },
        }
    }
}

impl Render for ReminderEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();
        let title = self.title.clone();
        let this_close = this.clone();
        let content = self.content.clone();
        let date = self.date.clone();
        let time = self.time.clone();
        let repeat = self.repeat.clone();

        div()
            .w(px(440.))
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
                    .child("新建提醒"),
            )
            .child(field_row(
                "提醒内容",
                Input::new(&title).h(px(32.)).w_full(),
            ))
            .child(repeat)
            .child(
                div()
                    .flex()
                    .gap_2()
                    .child(
                        div()
                            .flex_1()
                            .child(field_row("日期", Input::new(&date).h(px(32.)).w_full())),
                    )
                    .child(
                        div()
                            .flex_1()
                            .child(field_row("时间", Input::new(&time).h(px(32.)).w_full())),
                    ),
            )
            .child(
                div()
                    .w_full()
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .child(Textarea::new(&content).w_full()),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("日期格式 2026-03-05，时间格式 20:00。到点后会弹出系统通知。"),
            )
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .pt_1()
                    .child(
                        Button::new("reminder-cancel")
                            .secondary()
                            .compact()
                            .label("取消")
                            .on_click(move |_e, _w, app| {
                                close_editor(this_close.read(app).owner.clone(), app);
                            }),
                    )
                    .child(
                        Button::new("reminder-save")
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

pub fn apply_draft(db: &mut Database, draft: &ReminderDraft) {
    let due_at = format!(
        "{} {}",
        draft.due_date.trim(),
        normalize_time(&draft.due_time)
    );
    let mut reminder = match db.reminders.iter().find(|r| r.id == draft.id) {
        Some(existing) => existing.clone(),
        None => {
            let mut fresh = Reminder::new(draft.title.clone(), due_at.clone(), draft.repeat);
            if !draft.id.is_empty() {
                fresh.id = draft.id.clone();
            }
            fresh
        }
    };
    reminder.title = draft.title.clone();
    reminder.content = draft.content.clone();
    reminder.due_at = due_at;
    reminder.repeat = draft.repeat;
    reminder.done = false;
    db.save_reminder(reminder);
}

/// 把 "9:5" 之类的输入补全成 "09:05"。
fn normalize_time(text: &str) -> String {
    match parse_time(text) {
        Some(t) => model::format_time(t),
        None => "20:00".to_string(),
    }
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let pending = app.pending_reminder.clone();
    let show_done = app.show_done_reminders;
    let stamp = model::now_string();
    let today = model::today();

    let this_add = this.clone();
    let action: AnyElement = Button::new("add-reminder")
        .primary()
        .compact()
        .icon(Icon::new(IconName::Plus))
        .label("新建提醒")
        .on_click(move |_e, window, cx| {
            this_add.update(cx, |this, cx| {
                this.open_reminder_editor(ReminderDraft::empty(), window, cx);
            });
        })
        .into_any_element();

    let mut reminders: Vec<Reminder> = app.db.live_reminders().cloned().collect();
    reminders.sort_by(|a, b| a.done.cmp(&b.done).then_with(|| a.due_at.cmp(&b.due_at)));
    reminders.retain(|r| show_done || !r.done);

    let mut list = div().flex().flex_col().gap_2();
    if reminders.is_empty() {
        list = list.child(empty_state(
            &theme,
            IconName::Bell,
            "还没有提醒",
            Some("作业提交、交费、准备材料都可以设个提醒"),
        ));
    }

    for r in &reminders {
        let due = !r.done && is_due(r.due_at.trim(), &stamp);
        let overdue = due
            && r.due_at
                .get(11..16)
                .and_then(parse_time)
                .map(|t| model::time_minutes(t) + 60 < model::now_minutes())
                .unwrap_or(false);

        let accent = if r.done {
            theme.muted_foreground
        } else if overdue {
            theme.danger
        } else if due {
            theme.warning
        } else {
            theme.info
        };

        let id = r.id.clone();
        let id_a = r.id.clone();
        let id_b = r.id.clone();
        let this_toggle = this.clone();
        let this_edit = this.clone();
        let this_del = this.clone();
        let title = r.title.clone();
        let content = r.content.clone();
        let due_at = r.due_at.clone();
        let done = r.done;
        let repeat = r.repeat;

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
                        .id(eid("reminder-toggle", &r.id))
                        .mt(px(2.))
                        .flex_shrink_0()
                        .cursor_pointer()
                        .child(check_circle(&theme, done, px(20.)))
                        .on_click(move |_e, window, cx| {
                            this_toggle.update(cx, |this, cx| {
                                let done = this
                                    .db
                                    .live_reminders()
                                    .find(|x| x.id == id)
                                    .map(|x| x.done);
                                if let Some(done) = done {
                                    this.db.set_reminder_done(&id, !done);
                                    // 记账为设备本地态：切换后清除当日「已通知」，允许重新提醒。
                                    this.runtime.notified.remove(&id);
                                }
                                this.commit(window, cx);
                            });
                        }),
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
                                .flex()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .font_medium()
                                        .text_color(if done {
                                            theme.muted_foreground
                                        } else {
                                            theme.foreground
                                        })
                                        .child(title.clone()),
                                )
                                .child(chip(repeat.label(), accent, false))
                                .when(overdue, |d| d.child(chip("已超时", theme.danger, true))),
                        )
                        .when(!content.is_empty(), |d| {
                            d.child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(content.clone()),
                            )
                        })
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(due_at.clone()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .gap_1()
                        .child(action_button(
                            eid("reminder-edit", &r.id),
                            "编辑",
                            move |_e, window, cx| {
                                this_edit.update(cx, |this, cx| {
                                    let found =
                                        this.db.live_reminders().find(|x| x.id == id_a).cloned();
                                    if let Some(item) = found {
                                        let draft = ReminderDraft::from(&item);
                                        this.open_reminder_editor(draft, window, cx);
                                    }
                                });
                            },
                        ))
                        .child(action_button(
                            eid("reminder-del", &r.id),
                            "删除",
                            move |_e, window, cx| {
                                confirm_delete_reminder(this_del.clone(), window, cx, id_b.clone());
                            },
                        )),
                ),
        );
    }

    let today_count = app.db.reminders_on(today).len();

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap_4()
        .child(section_title(&theme, "提醒事项", Some(action)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(format!(
                            "共 {} 条 · 今天 {} 条",
                            app.db.live_reminder_count(),
                            today_count
                        )),
                )
                .child(
                    Button::new("toggle-done")
                        .ghost()
                        .compact()
                        .icon(Icon::new(if show_done {
                            IconName::EyeOff
                        } else {
                            IconName::Eye
                        }))
                        .label(if show_done {
                            "隐藏已完成"
                        } else {
                            "显示已完成"
                        })
                        .on_click(move |_e, _w, cx| {
                            this.update(cx, |this, cx| {
                                this.show_done_reminders = !this.show_done_reminders;
                                cx.notify();
                            });
                        }),
                ),
        )
        .child(div().flex_1().min_h_0().overflow_y_scrollbar().child(list))
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
fn confirm_delete_reminder(
    owner: Entity<App>,
    window: &mut Window,
    cx: &mut gpui::App,
    id: String,
) {
    window.open_alert_dialog(cx, move |alert, _w, _cx| {
        let owner = owner.clone();
        let id = id.clone();
        alert
            .title("删除提醒")
            .description("确定删除这条提醒吗？")
            .ok_text("删除")
            .ok_variant(ButtonVariant::Danger)
            .on_ok(move |_e, window, cx| {
                owner.update(cx, |this, cx| {
                    this.db.delete_reminder(&id);
                    this.commit(window, cx);
                });
                true
            })
    });
}
