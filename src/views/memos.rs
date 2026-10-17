//! 备忘笔记（DESIGN §4.7）。
//!
//! Phase 4 V3：`tab_chips`（全部 / 已置顶）、3 列卡片网格（左侧 4px 色条 +
//! 标题 + 多行省略正文 + 更新时间 + 置顶 chip + 置顶/编辑/删除动作）；
//! 编辑器走 `overlay` + `dialog`（§3.12）。
//!
//! 页面名、副标题与 `quick_add` 主按钮归顶栏（§3.1.4）——内容区只有 `section_title`。

use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::WindowExt;
use gpui::component::button::ButtonVariant;
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use std::rc::Rc;

use crate::app::App;
use crate::model::{Database, Memo};
use crate::ui::components::button as btn;
use crate::ui::components::empty_state::{EmptyVariant, empty_state};
use crate::ui::components::overlay::{dialog, overlay, section_title};
use crate::ui::components::tab_chips::tab_chips;
use crate::ui::components::tag::chip;
use crate::ui::form::field_row;
use crate::ui::palette::palette_at_for;
use crate::ui::tokens::{
    INPUT_H, RADIUS_LG, RADIUS_MD, SPACE_LG, SPACE_MD, SPACE_SM, SPACE_XL, SPACE_XS,
};
use crate::ui::widgets::{close_editor, eid, panel};

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

        // 8 色课程盘选色（§2.6：Phase 4 必须走 `palette_at_for`）。
        let mut swatches = div().flex().gap(SPACE_SM).flex_wrap();
        for i in 0..8usize {
            let active = i == self.color;
            let this = this.clone();
            swatches = swatches.child(
                div()
                    .id(eid("memo-color", &i.to_string()))
                    .size(px(24.))
                    .rounded_full()
                    .bg(palette_at_for(i, &theme))
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

        let body = div()
            .id(eid("memo-editor", "body"))
            .test_support()
            .flex()
            .flex_col()
            .gap(SPACE_LG)
            .child(Input::new(&title).h(INPUT_H).w_full())
            .child(
                div()
                    .w_full()
                    .min_h(px(160.))
                    .rounded(RADIUS_MD)
                    .border_1()
                    .border_color(theme.border)
                    .child(Textarea::new(&content).w_full()),
            )
            .child(field_row("颜色", swatches));

        let footer = div()
            .flex()
            .gap(SPACE_SM)
            .child(btn::secondary(
                "memo-cancel",
                "取消",
                move |_e, _w, app| {
                    close_editor(this_close.read(app).owner.clone(), app);
                },
            ))
            .child(btn::primary(
                "memo-save",
                "保存",
                move |_e, window, app| {
                    let (draft, cb) = {
                        let guard = this.read(app);
                        (guard.snapshot(app), guard.on_save.clone())
                    };
                    cb(draft, window, app);
                },
            ));

        // 注意：`overlay` 必须直接挂在页面根节点下（它是 `absolute inset_0` 的
        // 全屏遮罩——外层再包一个 div 会让它相对那个 0 高度的容器定位）。
        // 末尾 `.into_any_element()`：`overlay`/`dialog` 的 `impl IntoElement`
        // 在 Rust 2024 下会捕获 `&theme` 的生命周期，必须在此擦除（'static）后返回。
        overlay(&theme, dialog(&theme, "编辑备忘", body, Some(footer))).into_any_element()
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

    // 全部 / 已置顶（§3.5 tab_chips）。
    let this_tab = this.clone();
    let tabs = tab_chips(
        &theme,
        "memo-tab",
        &[("全部", MemoFilter::All), ("已置顶", MemoFilter::Pinned)],
        filter,
        move |value, _w, cx| {
            this_tab.update(cx, |this, cx| {
                this.memo_filter = value;
                cx.notify();
            });
        },
    );

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

    let section = match filter {
        MemoFilter::All => "全部备忘",
        MemoFilter::Pinned => "已置顶备忘",
    };
    let header_trailing = div()
        .flex()
        .items_center()
        .gap(SPACE_LG)
        .child(
            div()
                .text_size(px(12.))
                .text_color(theme.muted_foreground)
                .child(format!("共 {} 条", app.db.live_memo_count())),
        )
        .child(tabs)
        .into_any_element();

    let body: AnyElement = if memos.is_empty() {
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .items_center()
            .justify_center()
            .child(empty_state(
                &theme,
                EmptyVariant::Illustrated,
                IconName::NotebookPen,
                if filter == MemoFilter::Pinned {
                    "还没有置顶的备忘"
                } else {
                    "还没有备忘"
                },
                Some(if filter == MemoFilter::Pinned {
                    "点备忘卡片上的「置顶」，重要的事就排在最前面"
                } else {
                    "记下考试范围、家长会要点、需要买的文具…"
                }),
                None::<Div>,
            ))
            .into_any_element()
    } else {
        let mut grid = div().grid().grid_cols(3).gap(SPACE_LG);
        for m in &memos {
            grid = grid.child(memo_card(&theme, this.clone(), m));
        }
        div()
            .flex_1()
            .min_h_0()
            .overflow_y_scrollbar()
            .child(grid)
            .into_any_element()
    };

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap(SPACE_XL)
        .child(section_title(&theme, section, Some(header_trailing)))
        .child(body)
        .when_some(pending, |d, e| d.child(e))
}

/// 备忘卡片：左侧 4px 色条（`palette_at_for`）+ 标题 + 正文（3 行省略）+
/// 更新时间 + 置顶 chip + 置顶 / 编辑 / 删除动作。
fn memo_card(theme: &Theme, this: Entity<App>, m: &Memo) -> impl IntoElement {
    let accent = palette_at_for(m.color, theme);

    let id_pin = m.id.clone();
    let id_edit = m.id.clone();
    let id_del = m.id.clone();
    let this_pin = this.clone();
    let this_edit = this.clone();
    let this_del = this.clone();
    let pinned = m.pinned;

    let header = div()
        .flex()
        .items_start()
        .justify_between()
        .gap(SPACE_SM)
        .child(
            div()
                .flex_1()
                .min_w_0()
                .text_size(px(15.))
                .font_weight(FontWeight(600.0))
                .text_color(theme.foreground)
                .line_clamp(2)
                .child(if m.title.is_empty() {
                    "（无标题）".to_string()
                } else {
                    m.title.clone()
                }),
        )
        .when(pinned, |d| d.child(chip(theme, "置顶", theme.warning)));

    let content = if m.content.trim().is_empty() {
        String::new()
    } else {
        m.content.clone()
    };

    let actions = div()
        .flex()
        .items_center()
        .gap(SPACE_XS)
        .child(btn::icon_only(
            eid("memo-pin", &m.id),
            IconName::Pin,
            if pinned { "取消置顶" } else { "置顶" },
            move |_e, window, cx| {
                this_pin.update(cx, |this, cx| {
                    let found = this.db.live_memos().find(|x| x.id == id_pin).cloned();
                    if let Some(mut item) = found {
                        item.pinned = !item.pinned;
                        this.db.save_memo(item);
                    }
                    this.commit(window, cx);
                });
            },
        ))
        .child(btn::icon_only(
            eid("memo-edit", &m.id),
            IconName::Pencil,
            "编辑",
            move |_e, window, cx| {
                this_edit.update(cx, |this, cx| {
                    let found = this.db.live_memos().find(|x| x.id == id_edit).cloned();
                    if let Some(item) = found {
                        let draft = MemoDraft::from(&item);
                        this.open_memo_editor(draft, window, cx);
                    }
                });
            },
        ))
        .child(btn::icon_only(
            eid("memo-del", &m.id),
            IconName::Trash,
            "删除",
            move |_e, window, cx| {
                confirm_delete_memo(this_del.clone(), window, cx, id_del.clone());
            },
        ));

    div()
        .id(eid("memo-card", &m.id))
        .flex()
        .overflow_hidden()
        .rounded(RADIUS_LG)
        .border_1()
        .border_color(theme.border)
        .bg(panel(theme))
        .hover(|s| s.border_color(theme.input))
        .test_support()
        .child(div().w(px(4.)).flex_shrink_0().bg(accent))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(SPACE_MD)
                .p(SPACE_LG)
                .child(header)
                .child(
                    div()
                        .flex_1()
                        .min_h(px(56.))
                        .text_size(px(12.))
                        .text_color(theme.muted_foreground)
                        .line_clamp(3)
                        .child(content),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .justify_between()
                        .gap(SPACE_SM)
                        .child(
                            div()
                                .text_size(px(12.))
                                .text_color(theme.muted_foreground)
                                .child(m.updated_at.clone()),
                        )
                        .child(actions),
                ),
        )
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
