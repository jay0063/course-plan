//! 提醒事项（DESIGN §4.5）。
//!
//! Phase 4 V3：顶部 `tab_chips`（待办 / 已完成，承接原 `show_done_reminders`
//! 开关）、列表行语义圆点（`Once` = warning、循环 = info）、逾期左侧 warning 边、
//! 编辑 / 删除图标动作；编辑器走 `overlay` + `dialog`，日期与时间分别为
//! gpui-kit `DatePicker` / `TimeField`（§3.12、§3.15）。
//!
//! 页面名、副标题与 `quick_add` 主按钮归顶栏（§3.1.4）——内容区只有 `section_title`。

use crate::prelude::*;
use chrono::{NaiveDate, NaiveDateTime, NaiveTime};
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::WindowExt;
use gpui::component::button::ButtonVariant;
use gpui::component::date_picker::{DatePicker, DatePickerState};
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::component::time_field::{TimeField, TimeFieldState, TimePrecision};
use gpui::*;

use std::rc::Rc;

use crate::app::{App, is_due};
use crate::model::{self, Database, Reminder, Repeat, parse_time};
use crate::ui::components::button as btn;
use crate::ui::components::check_circle::check_circle;
use crate::ui::components::empty_state::{EmptyVariant, empty_state};
use crate::ui::components::list_row::list_row;
use crate::ui::components::overlay::{dialog, overlay, section_title};
use crate::ui::components::tab_chips::tab_chips;
use crate::ui::components::tag::chip;
use crate::ui::form::field_row;
use crate::ui::tokens::{
    INPUT_H, RADIUS_LG, RADIUS_MD, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XL, SPACE_XS,
};
use crate::ui::widgets::{close_editor, eid, panel};

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
    date: Entity<DatePickerState>,
    time: Entity<TimeFieldState>,
    repeat: Repeat,
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
        let due = parse_due(&draft.due_date, &draft.due_time);
        let due_time = due.time();

        let editor = Self {
            owner,
            draft: draft.clone(),
            title: cx
                .new(|cx| InputState::new(window, cx).placeholder("提醒内容，如：提交数学作业")),
            content: cx.new(|cx| TextareaState::new(window, cx).placeholder("补充说明")),
            date: cx.new(|cx| DatePickerState::new(window, cx).date_format("%Y-%m-%d")),
            time: cx.new(|cx| TimeFieldState::new(window, cx).precision(TimePrecision::Minute)),
            repeat: draft.repeat,
            on_save: Rc::new(on_save),
        };

        editor.title.update(cx, |x, cx| x.set_value(t, window, cx));
        editor
            .date
            .update(cx, |x, cx| x.set_date_time(due, window, cx));
        editor
            .time
            .update(cx, |x, cx| x.set_time(due_time, window, cx));
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
            due_date: self
                .date
                .read(cx)
                .date_time()
                .format("%Y-%m-%d")
                .map(|s| s.to_string())
                .unwrap_or_else(|| self.draft.due_date.clone()),
            due_time: model::format_time(self.time.read(cx).time()),
            repeat: self.repeat,
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

        // 重复方式：三段式 tab_chips，选项少而明确（§3.5、§3.15）。
        let this_repeat = this.clone();
        let repeat_tabs = tab_chips(
            &theme,
            "reminder-repeat",
            &[
                ("仅一次", Repeat::Once),
                ("每天", Repeat::Daily),
                ("每周", Repeat::Weekly),
            ],
            self.repeat,
            move |value, _w, cx| {
                this_repeat.update(cx, |this, cx| {
                    this.repeat = value;
                    cx.notify();
                });
            },
        );

        let body = div()
            .id(eid("reminder-editor", "body"))
            .test_support()
            .flex()
            .flex_col()
            .gap(SPACE_LG)
            .child(field_row(
                "提醒内容",
                Input::new(&title).h(INPUT_H).w_full(),
            ))
            .child(field_row(
                "日期时间",
                div()
                    .flex()
                    .items_center()
                    .gap(SPACE_MD)
                    .child(
                        div()
                            .id(eid("reminder-editor", "date"))
                            .flex_1()
                            .min_w_0()
                            .test_support()
                            .child(
                                DatePicker::new(&date)
                                    .placeholder("选择日期")
                                    .with_size(Size::Medium),
                            ),
                    )
                    .child(TimeField::new(&time).with_size(Size::Medium)),
            ))
            .child(field_row("重复", div().flex().child(repeat_tabs)))
            .child(field_row(
                "备注",
                div()
                    .w_full()
                    .rounded(RADIUS_MD)
                    .border_1()
                    .border_color(theme.border)
                    .child(Textarea::new(&content).w_full()),
            ))
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(theme.muted_foreground)
                    .child("日期与时间点选即可；到点后会弹出系统通知。"),
            );

        let footer = div()
            .flex()
            .gap(SPACE_SM)
            .child(btn::secondary(
                "reminder-cancel",
                "取消",
                move |_e, _w, app| {
                    close_editor(this_close.read(app).owner.clone(), app);
                },
            ))
            .child(btn::primary(
                "reminder-save",
                "保存",
                move |_e, window, app| {
                    let (draft, cb) = {
                        let guard = this.read(app);
                        (guard.snapshot(app), guard.on_save.clone())
                    };
                    cb(draft, window, app);
                },
            ));

        let dialog_title = if self.draft.id.is_empty() {
            "新建提醒"
        } else {
            "编辑提醒"
        };
        // 注意：`overlay` 必须直接挂在页面根节点下（它是 `absolute inset_0` 的
        // 全屏遮罩——外层再包一个 div 会让它相对那个 0 高度的容器定位）。
        // 末尾 `.into_any_element()`：`overlay`/`dialog` 的 `impl IntoElement`
        // 在 Rust 2024 下会捕获 `&theme` 的生命周期，必须在此擦除（'static）后返回。
        overlay(&theme, dialog(&theme, dialog_title, body, Some(footer))).into_any_element()
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

/// 草稿里的「日期 + 时间」文本 → `NaiveDateTime`（供 DatePicker / TimeField 初值）。
fn parse_due(date: &str, time: &str) -> NaiveDateTime {
    let date =
        NaiveDate::parse_from_str(date.trim(), "%Y-%m-%d").unwrap_or_else(|_| model::today());
    let time = parse_time(time).unwrap_or(NaiveTime::MIN);
    date.and_time(time)
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let pending = app.pending_reminder.clone();
    let show_done = app.show_done_reminders;
    let stamp = model::now_string();
    let today = model::today();

    // 待办 / 已完成（原「显示已完成」ghost 开关 → §3.5 tab_chips）。
    let this_tab = this.clone();
    let tabs = tab_chips(
        &theme,
        "reminder-tab",
        &[("待办", false), ("已完成", true)],
        show_done,
        move |done_tab, _w, cx| {
            this_tab.update(cx, |this, cx| {
                this.show_done_reminders = done_tab;
                cx.notify();
            });
        },
    );

    let mut reminders: Vec<Reminder> = app.db.live_reminders().cloned().collect();
    reminders.sort_by(|a, b| a.done.cmp(&b.done).then_with(|| a.due_at.cmp(&b.due_at)));
    reminders.retain(|r| if show_done { r.done } else { !r.done });

    let mut rows = div().flex().flex_col().gap(SPACE_XS);
    for r in &reminders {
        rows = rows.child(reminder_row(&theme, this.clone(), r, &stamp));
    }

    let section = if show_done {
        "已完成提醒"
    } else {
        "待办提醒"
    };
    let today_count = app.db.reminders_on(today).len();
    let count = if show_done {
        format!("已完成 {} 条", reminders.len())
    } else {
        format!(
            "共 {} 条 · 今天 {} 条",
            app.db.live_reminder_count(),
            today_count
        )
    };
    let header_trailing = div()
        .flex()
        .items_center()
        .gap(SPACE_LG)
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .child(count),
        )
        .child(tabs)
        .into_any_element();

    let list: AnyElement = if reminders.is_empty() {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .child(empty_state(
                &theme,
                EmptyVariant::Illustrated,
                IconName::Bell,
                if show_done {
                    "还没有已完成的提醒"
                } else {
                    "还没有提醒"
                },
                Some(if show_done {
                    "勾选一条提醒后，它会出现在这里"
                } else {
                    "作业提交、交费、准备材料都可以设个提醒"
                }),
                None::<Div>,
            ))
            .into_any_element()
    } else {
        div()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .p(SPACE_SM)
                    .gap(SPACE_XS)
                    .rounded(RADIUS_LG)
                    .border_1()
                    .border_color(theme.border)
                    .bg(panel(&theme))
                    .child(rows),
            )
            .into_any_element()
    };

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap(SPACE_XL)
        .child(section_title(&theme, section, Some(header_trailing)))
        .child(list)
        .when_some(pending, |d, e| d.child(e))
}

/// 单条提醒：语义圆点 + 勾选 + 标题/预览/到期 + 重复标签 + 编辑/删除。
///
/// 逾期（`due_at` 已过且未完成）时行左侧加 3px `warning` 边；超过 1 小时额外
/// 显示「已超时」标签（保留 Phase 2/3 的宽限语义）。
fn reminder_row(theme: &Theme, this: Entity<App>, r: &Reminder, stamp: &str) -> impl IntoElement {
    let due = !r.done && is_due(r.due_at.trim(), stamp);
    let overdue = due
        && r.due_at
            .get(11..16)
            .and_then(parse_time)
            .map(|t| model::time_minutes(t) + 60 < model::now_minutes())
            .unwrap_or(false);

    // 语义圆点：仅一次的提醒 = warning，循环提醒 = info；已完成转 muted。
    let dot_color = if r.done {
        theme.muted_foreground
    } else if r.repeat == Repeat::Once {
        theme.warning
    } else {
        theme.info
    };

    let id_toggle = r.id.clone();
    let this_toggle = this.clone();
    let done = r.done;
    let toggle = div()
        .id(eid("reminder-toggle", &r.id))
        .flex_shrink_0()
        .flex()
        .items_center()
        .cursor_pointer()
        .child(check_circle(theme, done, px(20.)))
        .test_support()
        .on_click(move |_e, window, cx| {
            this_toggle.update(cx, |this, cx| {
                let done = this
                    .db
                    .live_reminders()
                    .find(|x| x.id == id_toggle)
                    .map(|x| x.done);
                if let Some(done) = done {
                    this.db.set_reminder_done(&id_toggle, !done);
                    // 记账为设备本地态：切换后清除当日「已通知」，允许重新提醒。
                    this.runtime.notified.remove(&id_toggle);
                }
                this.commit(window, cx);
            });
        });

    let leading = div()
        .flex()
        .items_center()
        .gap(SPACE_SM)
        .child(toggle)
        .child(div().size(px(8.)).rounded_full().bg(dot_color));

    let preview = preview_line(&r.content);
    let due_text = due_display(&r.due_at);
    let subtitle = if preview.is_empty() {
        due_text
    } else {
        format!("{preview} · {due_text}")
    };

    let id_edit = r.id.clone();
    let this_edit = this.clone();
    let id_del = r.id.clone();
    let this_del = this.clone();
    let trailing = div()
        .flex()
        .items_center()
        .gap(SPACE_SM)
        .child(chip(theme, r.repeat.label(), dot_color))
        .when(overdue, |d| d.child(chip(theme, "已超时", theme.warning)))
        .child(btn::icon_only(
            eid("reminder-edit", &r.id),
            IconName::Pencil,
            "编辑",
            move |_e, window, cx| {
                this_edit.update(cx, |this, cx| {
                    let found = this.db.live_reminders().find(|x| x.id == id_edit).cloned();
                    if let Some(item) = found {
                        let draft = ReminderDraft::from(&item);
                        this.open_reminder_editor(draft, window, cx);
                    }
                });
            },
        ))
        .child(btn::icon_only(
            eid("reminder-del", &r.id),
            IconName::Trash,
            "删除",
            move |_e, window, cx| {
                confirm_delete_reminder(this_del.clone(), window, cx, id_del.clone());
            },
        ));

    let row = list_row(
        theme,
        eid("reminder-row", &r.id),
        None::<fn(&ClickEvent, &mut Window, &mut gpui::App)>,
        Some(leading),
        r.title.clone(),
        Some(subtitle),
        Some(trailing),
    );

    // 逾期左侧边：所有行保留同一几何（透明边占位），只有逾期行显示 warning。
    div()
        .border_l(px(3.))
        .border_color(if due {
            theme.warning
        } else {
            Hsla {
                a: 0.0,
                ..theme.foreground
            }
        })
        .pl(SPACE_XS)
        .when(r.done, |d| d.opacity(0.7))
        .child(row)
}

/// 内容预览：取第一行非空文本，超长按字符截断（12 字宽列表行只放一行）。
fn preview_line(text: &str) -> String {
    let line = text
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("")
        .trim();
    let mut out: String = line.chars().take(42).collect();
    if line.chars().count() > 42 {
        out.push('…');
    }
    out
}

/// 到期展示：能解析时用「今天 20:00」这类相对日，否则原样显示。
fn due_display(due_at: &str) -> String {
    let date = NaiveDate::parse_from_str(due_at.get(..10).unwrap_or("").trim(), "%Y-%m-%d").ok();
    match (date, due_at.get(11..16)) {
        (Some(date), Some(time)) => format!("{} {}", model::relative_day(date), time),
        _ => due_at.trim().to_string(),
    }
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
