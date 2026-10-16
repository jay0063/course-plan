use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::WindowExt;
use gpui::component::button::{Button, ButtonVariant, ButtonVariants};
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use std::rc::Rc;

use crate::app::App;
use crate::model::{Database, Memo};
use crate::ui::widgets::{
    PALETTE, action_button, close_editor, eid, empty_state, palette_at, panel, section_title,
};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MemoFilter {
    All,
    Pinned,
}

#[derive(Clone)]
pub struct MemoDraft {
    pub id: String,
    pub title: String,
    pub content: String,
    pub color: usize,
    pub pinned: bool,
}

impl MemoDraft {
    pub fn empty() -> Self {
        Self {
            id: crate::model::new_id(),
            title: String::new(),
            content: String::new(),
            color: 0,
            pinned: false,
        }
    }

    pub fn from(m: &Memo) -> Self {
        Self {
            id: m.id.clone(),
            title: m.title.clone(),
            content: m.content.clone(),
            color: m.color,
            pinned: m.pinned,
        }
    }
}

type SaveCb = Rc<dyn Fn(MemoDraft, &mut Window, &mut gpui::App)>;

pub struct MemoEditor {
    owner: Entity<App>,
    draft: MemoDraft,
    title: Entity<InputState>,
    content: Entity<TextareaState>,
    color: usize,
    on_save: SaveCb,
}

impl MemoEditor {
    pub fn new(
        draft: &MemoDraft,
        window: &mut Window,
        cx: &mut Context<App>,
        on_save: impl Fn(MemoDraft, &mut Window, &mut gpui::App) + 'static,
    ) -> Entity<Self> {
        let owner = cx.entity();
        let t = draft.title.clone();
        let c = draft.content.clone();

        let editor = Self {
            owner,
            draft: draft.clone(),
            title: cx.new(|cx| InputState::new(window, cx).placeholder("标题")),
            content: cx.new(|cx| TextareaState::new(window, cx).placeholder("写点什么…")),
            color: draft.color,
            on_save: Rc::new(on_save),
        };

        editor.title.update(cx, |x, cx| x.set_value(t, window, cx));
        editor.content.update(cx, |x, cx| {
            x.set_auto_grow(4, 12, cx);
            x.set_value(c, window, cx);
        });

        cx.new(|_| editor)
    }

    fn snapshot(&self, cx: &gpui::App) -> MemoDraft {
        MemoDraft {
            id: self.draft.id.clone(),
            title: self.title.read(cx).value().to_string(),
            content: self.content.read(cx).value().to_string(),
            color: self.color,
            pinned: self.draft.pinned,
        }
    }
}

impl Render for MemoEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();
        let title = self.title.clone();
        let this_close = this.clone();
        let content = self.content.clone();

        let mut swatches = div().flex().gap_2().flex_wrap();
        for i in 0..PALETTE.len() {
            let active = i == self.color;
            let this = this.clone();
            swatches = swatches.child(
                div()
                    .id(eid("memo-color", &i.to_string()))
                    .size(px(24.))
                    .rounded_full()
                    .bg(palette_at(i))
                    .border_2()
                    .border_color(if active {
                        theme.foreground
                    } else {
                        Hsla {
                            a: 0.0,
                            ..theme.foreground
                        }
                    })
                    .cursor_pointer()
                    .on_click(move |_e, _w, cx| {
                        this.update(cx, |this, cx| {
                            this.color = i;
                            cx.notify();
                        });
                    }),
            );
        }

        div()
            .w(px(480.))
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
                    .child("编辑备忘"),
            )
            .child(Input::new(&title).h(px(32.)).w_full())
            .child(
                div()
                    .w_full()
                    .min_h(px(200.))
                    .rounded_md()
                    .border_1()
                    .border_color(theme.border)
                    .child(Textarea::new(&content).w_full()),
            )
            .child(swatches)
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .pt_1()
                    .child(
                        Button::new("memo-cancel")
                            .secondary()
                            .compact()
                            .label("取消")
                            .on_click(move |_e, _w, app| {
                                close_editor(this_close.read(app).owner.clone(), app);
                            }),
                    )
                    .child(
                        Button::new("memo-save")
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

pub fn apply_draft(db: &mut Database, draft: &MemoDraft) {
    let mut memo = match db.memos.iter().find(|m| m.id == draft.id) {
        Some(existing) => existing.clone(),
        None => {
            let mut fresh = Memo::new();
            fresh.id = draft.id.clone();
            fresh.pinned = draft.pinned;
            fresh
        }
    };
    memo.title = draft.title.clone();
    memo.content = draft.content.clone();
    memo.color = draft.color;
    db.save_memo(memo);
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let pending = app.pending_memo.clone();
    let filter = app.memo_filter;

    let this_add = this.clone();
    let action: AnyElement = Button::new("add-memo")
        .primary()
        .compact()
        .icon(Icon::new(IconName::Plus))
        .label("新建备忘")
        .on_click(move |_e, window, cx| {
            this_add.update(cx, |this, cx| {
                this.open_memo_editor(MemoDraft::empty(), window, cx);
            });
        })
        .into_any_element();

    let mut memos: Vec<Memo> = app.db.live_memos().cloned().collect();
    match filter {
        MemoFilter::All => {}
        MemoFilter::Pinned => memos.retain(|m| m.pinned),
    }
    memos.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| b.updated_at.cmp(&a.updated_at))
    });

    let mut grid = div().grid().grid_cols(3).gap_3();
    if memos.is_empty() {
        return div()
            .flex_1()
            .min_h_0()
            .child(empty_state(
                &theme,
                IconName::NotebookPen,
                "还没有备忘",
                Some("记下考试范围、家长会要点、需要买的文具…"),
            ))
            .when_some(pending, |d, e| {
                d.child(
                    div()
                        .absolute()
                        .inset_0()
                        .bg(Hsla {
                            a: 0.45,
                            ..theme.foreground
                        })
                        .child(e),
                )
            });
    }
    for m in &memos {
        let _color = palette_at(m.color);
        let id = m.id.clone();
        let id_a = m.id.clone();
        let id_b = m.id.clone();
        let this_edit = this.clone();
        let this_pin = this.clone();
        let this_del = this.clone();
        let title = m.title.clone();
        let content = m.content.clone();
        let updated = m.updated_at.clone();
        let pinned = m.pinned;

        grid = grid.child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .p_3()
                .rounded_md()
                .border_1()
                .border_color(theme.border)
                .border_l_2()
                .bg(panel(&theme))
                .child(
                    div()
                        .flex()
                        .items_start()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .text_sm()
                                .font_semibold()
                                .text_color(theme.foreground)
                                .child(if title.is_empty() {
                                    "（无标题）".to_string()
                                } else {
                                    title.clone()
                                }),
                        )
                        .when(pinned, |d| {
                            d.child(
                                div()
                                    .flex_shrink_0()
                                    .px_2()
                                    .py(px(1.))
                                    .rounded_full()
                                    .border_1()
                                    .border_color(theme.warning)
                                    .text_xs()
                                    .text_color(theme.warning)
                                    .child("置顶"),
                            )
                        }),
                )
                .child(
                    div()
                        .flex_1()
                        .min_h(px(60.))
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(content.clone()),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .text_xs()
                                .text_color(theme.muted_foreground)
                                .child(updated.clone()),
                        )
                        .child(
                            div()
                                .flex()
                                .gap_1()
                                .child(action_button(
                                    eid("memo-pin", &m.id),
                                    if pinned { "取消置顶" } else { "置顶" },
                                    move |_e, window, cx| {
                                        this_pin.update(cx, |this, cx| {
                                            let found =
                                                this.db.live_memos().find(|x| x.id == id).cloned();
                                            if let Some(mut item) = found {
                                                item.pinned = !item.pinned;
                                                this.db.save_memo(item);
                                            }
                                            this.commit(window, cx);
                                        });
                                    },
                                ))
                                .child(action_button(
                                    eid("memo-edit", &m.id),
                                    "编辑",
                                    move |_e, window, cx| {
                                        this_edit.update(cx, |this, cx| {
                                            let found = this
                                                .db
                                                .live_memos()
                                                .find(|x| x.id == id_a)
                                                .cloned();
                                            if let Some(item) = found {
                                                let draft = MemoDraft::from(&item);
                                                this.open_memo_editor(draft, window, cx);
                                            }
                                        });
                                    },
                                ))
                                .child(action_button(
                                    eid("memo-del", &m.id),
                                    "删除",
                                    move |_e, window, cx| {
                                        confirm_delete_memo(
                                            this_del.clone(),
                                            window,
                                            cx,
                                            id_b.clone(),
                                        );
                                    },
                                )),
                        ),
                ),
        );
    }

    let grid = div().flex_1().min_h_0().overflow_y_scrollbar().child(grid);

    let mut tabs = div().flex().gap_2();
    for (label, f) in [("全部", MemoFilter::All), ("置顶", MemoFilter::Pinned)] {
        let active = filter == f;
        let this = this.clone();
        tabs = tabs.child(
            div()
                .id(eid("memo-filter", label))
                .px_3()
                .py(px(4.))
                .rounded_full()
                .text_xs()
                .cursor_pointer()
                .border_1()
                .border_color(if active { theme.primary } else { theme.border })
                .bg(if active {
                    theme.primary.with_alpha(0.12)
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
                .on_click(move |_e, _w, cx| {
                    this.update(cx, |this, cx| {
                        this.memo_filter = f;
                        cx.notify();
                    });
                })
                .child(label),
        );
    }

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap_4()
        .child(section_title(&theme, "备忘笔记", Some(action)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(tabs)
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(format!("共 {} 条", app.db.live_memo_count())),
                ),
        )
        .child(grid)
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
fn confirm_delete_memo(owner: Entity<App>, window: &mut Window, cx: &mut gpui::App, id: String) {
    window.open_alert_dialog(cx, move |alert, _w, _cx| {
        let owner = owner.clone();
        let id = id.clone();
        alert
            .title("删除备忘")
            .description("确定删除这条备忘吗？")
            .ok_text("删除")
            .ok_variant(ButtonVariant::Danger)
            .on_ok(move |_e, window, cx| {
                owner.update(cx, |this, cx| {
                    this.db.delete_memo(&id);
                    this.commit(window, cx);
                });
                true
            })
    });
}
