use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use std::rc::Rc;

use crate::app::App;
use crate::model::{self, CheckIn, Database, Repeat, WEEKDAY_SHORT};
use crate::ui::form::field_row;
use crate::ui::select::{Select, SelectOption};
use crate::ui::widgets::{
    action_button, check_circle, chip, close_editor, eid, panel, section_title,
};

#[derive(Clone)]
pub struct CheckInDraft {
    pub id: String,
    pub title: String,
    pub note: String,
    pub repeat: Repeat,
    pub weekdays: Vec<u8>,
    pub date: String,
}

impl CheckInDraft {
    pub fn empty() -> Self {
        Self {
            id: String::new(),
            title: String::new(),
            note: String::new(),
            repeat: Repeat::Daily,
            weekdays: vec![1, 2, 3, 4, 5],
            date: model::fmt_date(model::today()),
        }
    }

    pub fn from(c: &CheckIn) -> Self {
        Self {
            id: c.id.clone(),
            title: c.title.clone(),
            note: c.note.clone(),
            repeat: c.repeat,
            weekdays: c.weekdays.clone(),
            date: c.date.clone(),
        }
    }
}

type SaveCb = Rc<dyn Fn(CheckInDraft, &mut Window, &mut gpui::App)>;

pub struct CheckInEditor {
    owner: Entity<App>,
    draft: CheckInDraft,
    title: Entity<InputState>,
    note: Entity<TextareaState>,
    date: Entity<InputState>,
    repeat: Entity<Select>,
    weekday_flags: [bool; 7],
    on_save: SaveCb,
}

impl CheckInEditor {
    pub fn new(
        draft: &CheckInDraft,
        window: &mut Window,
        cx: &mut Context<App>,
        on_save: impl Fn(CheckInDraft, &mut Window, &mut gpui::App) + 'static,
    ) -> Entity<Self> {
        let owner = cx.entity();
        let t = draft.title.clone();
        let n = draft.note.clone();
        let d = draft.date.clone();
        let repeat_index = Some(match draft.repeat {
            Repeat::Once => 0,
            Repeat::Daily => 1,
            Repeat::Weekly => 2,
        });

        let mut flags = [false; 7];
        for w in &draft.weekdays {
            if let Some(slot) = flags.get_mut((*w as usize).clamp(1, 7) - 1) {
                *slot = true;
            }
        }

        let editor = Self {
            owner,
            draft: draft.clone(),
            title: cx
                .new(|cx| InputState::new(window, cx).placeholder("任务名称，如：阅读 20 分钟")),
            note: cx.new(|cx| TextareaState::new(window, cx).placeholder("说明")),
            date: cx.new(|cx| InputState::new(window, cx).placeholder("2026-03-05")),
            repeat: cx.new(|_| {
                Select::new("checkin-repeat")
                    .label("重复方式")
                    .options(vec![
                        SelectOption::new("仅一次"),
                        SelectOption::new("每天"),
                        SelectOption::new("每周指定"),
                    ])
                    .selected(repeat_index)
            }),
            weekday_flags: flags,
            on_save: Rc::new(on_save),
        };

        editor.title.update(cx, |x, cx| x.set_value(t, window, cx));
        editor.date.update(cx, |x, cx| x.set_value(d, window, cx));
        editor.note.update(cx, |x, cx| {
            x.set_auto_grow(2, 4, cx);
            x.set_value(n, window, cx);
        });

        cx.new(|_| editor)
    }

    fn snapshot(&self, cx: &gpui::App) -> CheckInDraft {
        CheckInDraft {
            id: self.draft.id.clone(),
            title: self.title.read(cx).value().to_string(),
            note: self.note.read(cx).value().to_string(),
            repeat: match self.repeat.read(cx).selected_index() {
                Some(0) => Repeat::Once,
                Some(2) => Repeat::Weekly,
                _ => Repeat::Daily,
            },
            weekdays: self
                .weekday_flags
                .iter()
                .enumerate()
                .filter(|(_, on)| **on)
                .map(|(i, _)| (i + 1) as u8)
                .collect(),
            date: self.date.read(cx).value().to_string(),
        }
    }
}

impl Render for CheckInEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();
        let title = self.title.clone();
        let this_close = this.clone();
        let note = self.note.clone();
        let date = self.date.clone();
        let repeat = self.repeat.clone();
        let is_weekly = self.repeat.read(cx).selected_index() == Some(2);
        let is_once = self.repeat.read(cx).selected_index() == Some(0);

        let mut day_row = div().flex().gap_1();
        for (i, flag) in self.weekday_flags.iter().enumerate() {
            let active = *flag;
            let this = this.clone();
            day_row = day_row.child(
                div()
                    .id(eid("weekday-toggle", &i.to_string()))
                    .w(px(30.))
                    .h(px(28.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded_sm()
                    .cursor_pointer()
                    .text_xs()
                    .border_1()
                    .border_color(if active { theme.primary } else { theme.border })
                    .bg(if active {
                        theme.primary.with_alpha(0.14)
                    } else {
                        Hsla {
                            a: 0.0,
                            ..theme.foreground
                        }
                    })
                    .text_color(if active {
                        theme.primary
                    } else {
                        theme.muted_foreground
                    })
                    .id(eid("checkin-weekday", &i.to_string()))
                    .on_click(move |_e, _w, cx| {
                        this.update(cx, |this, cx| {
                            this.weekday_flags[i] = !this.weekday_flags[i];
                            cx.notify();
                        });
                    })
                    .child(WEEKDAY_SHORT[i]),
            );
        }

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
                    .child("新建打卡任务"),
            )
            .child(field_row(
                "任务名称",
                Input::new(&title).h(px(32.)).w_full(),
            ))
            .child(repeat)
            .when(is_weekly, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(div().text_xs().font_medium().child("选择星期"))
                        .child(day_row),
                )
            })
            .when(is_once, |d| {
                d.child(field_row("指定日期", Input::new(&date).h(px(32.)).w_full()))
            })
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
                    .pt_1()
                    .child(
                        Button::new("checkin-cancel")
                            .secondary()
                            .compact()
                            .label("取消")
                            .on_click(move |_e, _w, app| {
                                close_editor(this_close.read(app).owner.clone(), app);
                            }),
                    )
                    .child(
                        Button::new("checkin-save")
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

pub fn apply_draft(db: &mut Database, draft: &CheckInDraft) {
    let mut checkin = match db.checkins.iter().find(|c| c.id == draft.id) {
        Some(existing) => existing.clone(),
        None => {
            let mut fresh = CheckIn::new(draft.title.clone(), draft.repeat);
            if !draft.id.is_empty() {
                fresh.id = draft.id.clone();
            }
            fresh
        }
    };
    checkin.title = draft.title.clone();
    checkin.note = draft.note.clone();
    checkin.repeat = draft.repeat;
    checkin.weekdays = draft.weekdays.clone();
    checkin.date = draft.date.clone();
    checkin.archived = false;
    db.save_checkin(checkin);
}

/// 看板分栏。
const COLUMNS: [(&str, usize); 3] = [("待打卡", 0), ("今日已完成", 1), ("全部任务", 2)];

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let pending = app.pending_checkin.clone();
    let today = model::today();

    let this_add = this.clone();
    let action: AnyElement = Button::new("add-checkin")
        .primary()
        .compact()
        .icon(Icon::new(IconName::Plus))
        .label("新建任务")
        .on_click(move |_e, window, cx| {
            this_add.update(cx, |this, cx| {
                this.open_checkin_editor(CheckInDraft::empty(), window, cx);
            });
        })
        .into_any_element();

    let all: Vec<CheckIn> = app
        .db
        .live_checkins()
        .filter(|c| !c.archived)
        .cloned()
        .collect();
    let active_today: Vec<CheckIn> = all.iter().filter(|c| c.active_on(today)).cloned().collect();
    let done_today: Vec<CheckIn> = active_today
        .iter()
        .filter(|c| c.done_on(today))
        .cloned()
        .collect();
    let pending_today: Vec<CheckIn> = active_today
        .iter()
        .filter(|c| !c.done_on(today))
        .cloned()
        .collect();

    let mut board = div().flex().gap_3().flex_1().min_h_0();

    for (title, which) in COLUMNS {
        let items: Vec<CheckIn> = match which {
            0 => pending_today.clone(),
            1 => done_today.clone(),
            _ => all.clone(),
        };

        let mut column = div()
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap_2()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(theme.border)
            .bg(panel(&theme))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_sm()
                            .font_semibold()
                            .text_color(theme.foreground)
                            .child(title),
                    )
                    .child(
                        div()
                            .px_2()
                            .py(px(1.))
                            .rounded_full()
                            .text_xs()
                            .bg(theme.muted.with_alpha(0.25))
                            .text_color(theme.muted_foreground)
                            .child(items.len().to_string()),
                    ),
            );

        if items.is_empty() {
            column = column.child(
                div()
                    .py_8()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child("暂无任务"),
            );
        }

        for c in &items {
            let done = c.done_on(today);
            let streak = c.streak();
            let total = c.total_done();
            let id = c.id.clone();
            let id_a = c.id.clone();
            let id_b = c.id.clone();
            let this_toggle = this.clone();
            let this_edit = this.clone();
            let this_del = this.clone();
            let task_title = c.title.clone();
            let note = c.note.clone();
            let repeat_label = c.repeat_label();

            column = column.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .rounded_md()
                    .border_1()
                    .border_color(if done {
                        theme.success.with_alpha(0.4)
                    } else {
                        theme.border
                    })
                    .bg(if done {
                        theme.success.with_alpha(0.08)
                    } else {
                        Hsla {
                            a: 0.0,
                            ..theme.foreground
                        }
                    })
                    .child(
                        div()
                            .flex()
                            .items_start()
                            .gap_2()
                            .child(
                                div()
                                    .id(eid("checkin-toggle", &c.id))
                                    .mt(px(1.))
                                    .cursor_pointer()
                                    .child(check_circle(&theme, done, px(18.)))
                                    .on_click(move |_e, window, cx| {
                                        this_toggle.update(cx, |this, cx| {
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
                                    }),
                            )
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .text_color(if done {
                                        theme.muted_foreground
                                    } else {
                                        theme.foreground
                                    })
                                    .child(task_title.clone()),
                            ),
                    )
                    .when(!note.is_empty(), |d| {
                        d.child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(note.clone()),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .flex_wrap()
                            .child(chip(repeat_label.clone(), theme.info, false))
                            .when(streak > 0, |d| {
                                d.child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_1()
                                        .text_xs()
                                        .text_color(theme.warning)
                                        .child(Icon::new(IconName::Flame).with_size(Size::XSmall))
                                        .child(format!("{streak} 天")),
                                )
                            })
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(format!("累计 {total} 次")),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .gap_1()
                            .child(action_button(
                                eid("checkin-edit", &c.id),
                                "编辑",
                                move |_e, window, cx| {
                                    this_edit.update(cx, |this, cx| {
                                        let found =
                                            this.db.live_checkins().find(|x| x.id == id_a).cloned();
                                        if let Some(item) = found {
                                            let draft = CheckInDraft::from(&item);
                                            this.open_checkin_editor(draft, window, cx);
                                        }
                                    });
                                },
                            ))
                            .child(action_button(
                                eid("checkin-arch", &c.id),
                                "归档",
                                move |_e, window, cx| {
                                    this_del.update(cx, |this, cx| {
                                        this.db.set_checkin_archived(&id_b, true);
                                        this.commit(window, cx);
                                    });
                                },
                            )),
                    ),
            );
        }

        board = board.child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_y_scrollbar()
                .child(column),
        );
    }

    // 归档区
    let archived: Vec<CheckIn> = app
        .db
        .live_checkins()
        .filter(|c| c.archived)
        .cloned()
        .collect();
    let this_unarchive = this.clone();
    let mut archive_row = div().flex().gap_2().flex_wrap();
    for c in &archived {
        let id = c.id.clone();
        let title = c.title.clone();
        let this = this_unarchive.clone();
        archive_row = archive_row.child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_2()
                .py(px(4.))
                .rounded_full()
                .border_1()
                .border_color(theme.border)
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(title)
                .child(
                    div()
                        .cursor_pointer()
                        .text_color(theme.primary)
                        .id(eid("checkin-restore", &id))
                        .on_click(move |_e, window, cx| {
                            this.update(cx, |this, cx| {
                                this.db.set_checkin_archived(&id, false);
                                this.commit(window, cx);
                            });
                        })
                        .child("恢复"),
                ),
        );
    }

    let total_rate = if active_today.is_empty() {
        0
    } else {
        done_today.len() * 100 / active_today.len()
    };

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap_4()
        .child(section_title(&theme, "打卡任务", Some(action)))
        .child(
            div()
                .flex()
                .items_center()
                .gap_3()
                .px_3()
                .py_2()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .bg(panel(&theme))
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.foreground)
                        .child(format!("今日完成率 {total_rate}%")),
                )
                .child(
                    div()
                        .flex_1()
                        .h(px(6.))
                        .rounded_full()
                        .bg(theme.muted.with_alpha(0.3))
                        .child(
                            div()
                                .h(px(6.))
                                .rounded_full()
                                .w(relative_len(total_rate as f32 / 100.0))
                                .bg(theme.success),
                        ),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(format!("{}/{}", done_today.len(), active_today.len())),
                ),
        )
        .child(board)
        .when(!archived.is_empty(), |d| {
            d.child(
                div()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child("已归档"),
                    )
                    .child(archive_row),
            )
        })
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

/// 0.0 - 1.0 的进度条宽度。
fn relative_len(ratio: f32) -> Length {
    gpui::relative(ratio.clamp(0.0, 1.0)).into()
}
