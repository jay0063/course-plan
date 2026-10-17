//! 打卡任务页（DESIGN §4.6 / §3.17）。
//!
//! 结构：`section_title("今日打卡")` → 完成率 progress 卡 → 3 列看板
//! （待打卡 / 今日已完成 / 全部任务）→ 已归档 chips；编辑器与归档确认走
//! §3.12 overlay + dialog / alert_dialog（全屏遮罩、唯一 id、可取消）。
//!
//! 页面名与「新建任务」主按钮只属于顶栏（§3.1.4）——内容区不再出现。

use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::Duration;

use crate::app::App;
use crate::gpui::base::Easing;
use crate::model::{self, CheckIn, Database, Repeat, WEEKDAY_SHORT};
use crate::ui::components::button;
use crate::ui::components::card::{CardVariant, card_with_theme};
use crate::ui::components::check_circle::check_circle;
use crate::ui::components::empty_state::{EmptyVariant, empty_state};
use crate::ui::components::overlay::{AlertVariant, alert_dialog, dialog, overlay, section_title};
use crate::ui::components::panel::panel;
use crate::ui::components::progress::{progress, streak_badge};
use crate::ui::components::tag::{badge, chip};
use crate::ui::form::field_row;
use crate::ui::motion;
use crate::ui::select::{Select, SelectOption};
use crate::ui::tokens::text as text_tokens;
use crate::ui::tokens::{
    INPUT_H, RADIUS_FULL, RADIUS_LG, RADIUS_SM, SPACE_2XL, SPACE_MD, SPACE_SM, SPACE_XL, SPACE_XS,
};
use crate::ui::widgets::{close_editor, eid};

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

/// 编辑器里星期开关的尺寸（一次性像素：形态开关，非 token 化间距）。
const WEEKDAY_CHIP_W: f32 = 34.;
const WEEKDAY_CHIP_H: f32 = 30.;

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
        let note = self.note.clone();
        let date = self.date.clone();
        let repeat = self.repeat.clone();
        let is_weekly = self.repeat.read(cx).selected_index() == Some(2);
        let is_once = self.repeat.read(cx).selected_index() == Some(0);
        let heading = if self.draft.id.is_empty() {
            "新建打卡任务"
        } else {
            "编辑打卡任务"
        };

        let mut day_row = div().flex().gap(SPACE_XS);
        for (i, flag) in self.weekday_flags.iter().enumerate() {
            let active = *flag;
            let this_day = this.clone();
            day_row = day_row.child(
                div()
                    .id(eid("checkin-weekday", &i.to_string()))
                    .w(px(WEEKDAY_CHIP_W))
                    .h(px(WEEKDAY_CHIP_H))
                    .flex()
                    .items_center()
                    .justify_center()
                    .rounded(RADIUS_SM)
                    .cursor_pointer()
                    .text_size(text_tokens::CAPTION)
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
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .hover(|s| s.bg(theme.list_hover))
                    .on_click(move |_e, _w, cx| {
                        this_day.update(cx, |this, cx| {
                            this.weekday_flags[i] = !this.weekday_flags[i];
                            cx.notify();
                        });
                    })
                    .child(WEEKDAY_SHORT[i]),
            );
        }

        let mut body = div()
            .flex()
            .flex_col()
            .gap(SPACE_MD)
            .child(field_row(
                "任务名称",
                Input::new(&title).h(INPUT_H).w_full(),
            ))
            .child(repeat)
            .when(is_weekly, |d| {
                d.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap(SPACE_XS)
                        .child(
                            div()
                                .text_size(text_tokens::CAPTION)
                                .text_color(theme.muted_foreground)
                                .child("选择星期"),
                        )
                        .child(day_row),
                )
            })
            .when(is_once, |d| {
                d.child(field_row("指定日期", Input::new(&date).h(INPUT_H).w_full()))
            })
            .child(
                div()
                    .w_full()
                    .rounded(RADIUS_LG)
                    .border_1()
                    .border_color(theme.input)
                    .child(Textarea::new(&note).w_full()),
            );
        body = body.child(
            div()
                .text_size(text_tokens::CAPTION)
                .text_color(theme.muted_foreground)
                .child("「每周指定」时按星期生效"),
        );

        let owner = self.owner.clone();
        let footer = div()
            .flex()
            .items_center()
            .gap(SPACE_SM)
            .child(button::secondary(
                "checkin-cancel",
                "取消",
                move |_e, _w, app| {
                    close_editor(owner.clone(), app);
                },
            ))
            .child(button::primary(
                "checkin-save",
                "保存",
                move |_e, window, app| {
                    let (draft, cb) = {
                        let guard = this.read(app);
                        (guard.snapshot(app), guard.on_save.clone())
                    };
                    cb(draft, window, app);
                },
            ));

        div().child(dialog(&theme, heading, body, Some(footer)))
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

// MARK: - §3.17 打卡庆祝

/// 勾选圆环固定的包围盒尺寸（弹跳在其内部发生，不推挤相邻文字）。
const CHECK_BOX: f32 = 30.;
/// 圆环常态直径。
const CHECK_DIAMETER: f32 = 20.;
/// 弹跳峰值额外直径。
const CHECK_PULSE_PX: f32 = 2.;
/// 「短促光晕」峰值透明度。
const CHECK_HALO_ALPHA: f32 = 0.20;
/// §3.17 庆祝背景淡入的目标透明度。
const CELEBRATION_ALPHA: f32 = 0.08;
/// 庆祝瞬间的驻留时长（§3.17：1200ms 后回淡）。
const CELEBRATION_MS: u64 = 1200;

thread_local! {
    /// 庆祝瞬间的世代号：0 = 无庆祝；每次触发递增，作为一次性动画的 id 后缀。
    static CELEBRATION: Cell<u64> = const { Cell::new(0) };
    /// 归档二次确认的目标任务 id（视图瞬态，不落库、不占用 App 字段）。
    static ARCHIVE_CONFIRM: RefCell<Option<String>> = const { RefCell::new(None) };
}

/// 打卡弹跳曲线（§3.17：一次性的「弹一下」）。
///
/// `ui::motion::bounce_spring()` 返回 gpui-base 的弹簧策略，只能交给
/// `gpui_base::motion::spring(id, target, policy, window, cx)` —— 视图的
/// `render(app, cx)` 拿不到 `&mut Window`，因此这里用等价的一次性
/// Animation（同量级时长 + ease_out_back 过冲）落地；reduce-motion 由
/// `with_animation` 自动落到终态。本 gpui 分支没有 CSS transform
/// （`scale()`/`rotate()`），弹跳以「圆环直径 + 光晕」表达。
pub(crate) fn bounce_animation(theme: &Theme) -> Animation {
    let duration = motion::tokens(theme).duration_slow;
    let curve = Easing::cubic_bezier(0.34, 1.56, 0.64, 1.0).expect("ease-out-back 控制点合法");
    Animation::new(duration).with_easing(move |t| curve.sample(t))
}

/// 把过冲后的曲线采样值折成「0 → 1 → 0」的脉冲（终态恒为 0）。
pub(crate) fn bounce_pulse(delta: f32) -> f32 {
    (1.0 - (2.0 * delta - 1.0).abs()).clamp(0.0, 1.0)
}

/// 打卡勾选圆环 + 短促光晕（§3.17：「spring 弹一下 + 短促光晕」）。
///
/// `anim_id` 必须随 done 状态变化，使每次勾选都重放一次弹跳。
pub(crate) fn check_burst(theme: &Theme, done: bool, anim_id: ElementId) -> impl IntoElement {
    let theme = theme.clone();
    let animation = bounce_animation(&theme);
    div().with_animation(anim_id, animation, move |_, delta| {
        let pulse = bounce_pulse(delta);
        div()
            .w(px(CHECK_BOX))
            .h(px(CHECK_BOX))
            .flex()
            .items_center()
            .justify_center()
            .rounded(RADIUS_FULL)
            .bg(theme.success.with_alpha(CHECK_HALO_ALPHA * pulse))
            .child(check_circle(
                &theme,
                done,
                px(CHECK_DIAMETER + CHECK_PULSE_PX * pulse),
            ))
    })
}

/// 当日全部打卡任务都已勾选（且至少有一项）。
fn all_today_done(db: &Database, today: chrono::NaiveDate) -> bool {
    let items = db.checkins_on(today);
    !items.is_empty() && items.iter().all(|c| c.done_on(today))
}

/// 触发一次庆祝瞬间（§3.17：全部完成 → 中心 Sparkle + success 淡入）。
fn trigger_celebration(cx: &mut Context<App>) {
    let tick = CELEBRATION.with(|c| {
        let next = c.get().wrapping_add(1).max(1);
        c.set(next);
        next
    });
    let view = cx.entity();
    cx.spawn(async move |_this, cx| {
        cx.background_executor()
            .timer(Duration::from_millis(CELEBRATION_MS))
            .await;
        // 同一个世代号被清掉后，下一次庆祝用新号重放动画。
        CELEBRATION.with(|c| {
            if c.get() == tick {
                c.set(0);
            }
        });
        view.update(cx, |_this, cx| cx.notify());
    })
    .detach();
    cx.notify();
}

/// 庆祝瞬间浮层：success 淡入 + 中心 Celebratory 空态（§3.11 / §3.17）。
fn celebration_layer(theme: &Theme, tick: u64) -> AnyElement {
    let tint_theme = theme.clone();
    let fade_in = Animation::new(motion::tokens(theme).duration_slow * 2);
    let tint = div()
        .with_animation(
            ElementId::from(format!("checkin-celebration-tint:{tick}")),
            fade_in,
            move |_, delta| {
                div()
                    .absolute()
                    .inset_0()
                    .bg(tint_theme.success.with_alpha(CELEBRATION_ALPHA * delta))
            },
        )
        .into_any_element();

    let burst_theme = theme.clone();
    let appear = Animation::new(motion::tokens(theme).duration_slow);
    let burst = div()
        .with_animation(
            ElementId::from(format!("checkin-celebration-burst:{tick}")),
            appear,
            move |_, delta| {
                // 庆祝块自带纸面（Hero 卡），叠在忙碌的看板之上仍然可读。
                div().opacity(delta).child(card_with_theme(
                    &burst_theme,
                    CardVariant::Hero,
                    None::<SharedString>,
                    None::<SharedString>,
                    empty_state(
                        &burst_theme,
                        EmptyVariant::Celebratory,
                        IconName::Sparkle,
                        "今日任务全部完成",
                        Some("做得不错！"),
                        None::<AnyElement>,
                    ),
                ))
            },
        )
        .into_any_element();

    div()
        .absolute()
        .inset_0()
        .child(tint)
        .child(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .child(burst),
        )
        .into_any_element()
}

// MARK: - 看板

/// 看板分栏（`key` 用于生成每列唯一的 test id——同一任务会同时出现在多列）。
const COLUMNS: [(&str, usize, &str); 3] = [
    ("待打卡", 0, "pending"),
    ("今日已完成", 1, "done"),
    ("全部任务", 2, "all"),
];

/// 任务卡片（§4.6）：check_circle + 标题 + 描述 + repeat chip + weekday chip
/// + streak + 编辑/归档 图标按钮。
fn task_card(
    theme: &Theme,
    view: &Entity<App>,
    item: &CheckIn,
    today: chrono::NaiveDate,
    col_key: &str,
) -> AnyElement {
    let done = item.done_on(today);
    let streak = item.streak();
    let total = item.total_done();
    let id = item.id.clone();

    // MARK: 元信息 chips（repeat / weekday / streak / 累计）
    let mut meta = div().flex().items_center().gap(SPACE_SM).flex_wrap();
    match item.repeat {
        Repeat::Daily => meta = meta.child(chip(theme, "每天", theme.info)),
        Repeat::Once => {
            meta = meta.child(chip(theme, "仅一次", theme.info));
            meta = meta.child(chip(theme, item.date.clone(), theme.muted_foreground));
        }
        Repeat::Weekly => {
            meta = meta.child(chip(theme, "每周", theme.info));
            let mut days = item.weekdays.clone();
            days.sort();
            for w in days {
                let label = WEEKDAY_SHORT[(w as usize).clamp(1, 7) - 1];
                meta = meta.child(chip(theme, label, theme.accent));
            }
        }
    }
    if streak > 0 {
        meta = meta.child(streak_badge(theme, streak));
    }
    meta = meta.child(
        div()
            .text_size(text_tokens::CAPTION)
            .text_color(theme.muted_foreground)
            .child(format!("累计 {total} 次")),
    );

    // MARK: 勾选（点击 → 翻转 done + §3.17 弹跳）
    let this_toggle = view.clone();
    let id_toggle = id.clone();
    let toggle = div()
        .id(eid(&format!("checkin-toggle-{col_key}"), &id))
        .cursor_pointer()
        .test_support()
        .child(check_burst(
            theme,
            done,
            ElementId::from(format!("checkin-burst:{col_key}:{id}:{done}")),
        ))
        .on_click(move |_e, window, cx| {
            let id = id_toggle.clone();
            this_toggle.update(cx, |this, cx| {
                let was = this
                    .db
                    .live_checkins()
                    .find(|x| x.id == id)
                    .map(|x| x.done_on(today));
                if let Some(was) = was {
                    this.db.set_checkin_done(&id, today, !was);
                    if !was && all_today_done(&this.db, today) {
                        trigger_celebration(cx);
                    }
                }
                this.commit(window, cx);
            });
        });

    // MARK: 编辑 / 归档
    let this_edit = view.clone();
    let id_edit = id.clone();
    let edit_btn = button::icon_only(
        eid(&format!("checkin-edit-{col_key}"), &id),
        IconName::Pencil,
        "编辑",
        move |_e, window, cx| {
            let id = id_edit.clone();
            this_edit.update(cx, |this, cx| {
                let found = this.db.live_checkins().find(|x| x.id == id).cloned();
                if let Some(item) = found {
                    this.open_checkin_editor(CheckInDraft::from(&item), window, cx);
                }
            });
        },
    );

    let this_arch = view.clone();
    let id_arch = id.clone();
    let archive_btn = button::icon_only(
        eid(&format!("checkin-arch-{col_key}"), &id),
        IconName::Archive,
        "归档",
        move |_e, _w, cx| {
            let id = id_arch.clone();
            ARCHIVE_CONFIRM.with(|c| *c.borrow_mut() = Some(id));
            this_arch.update(cx, |_this, cx| cx.notify());
        },
    );

    let title_color = if done {
        theme.muted_foreground
    } else {
        theme.foreground
    };

    card_with_theme(
        theme,
        CardVariant::Default,
        None::<SharedString>,
        None::<SharedString>,
        div()
            .flex()
            .flex_col()
            .gap(SPACE_SM)
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(SPACE_SM)
                    .child(toggle)
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .pt(px(4.))
                            .text_size(text_tokens::BODY_EMPH)
                            .font_weight(FontWeight(600.))
                            .text_color(title_color)
                            .child(item.title.clone()),
                    ),
            )
            .when(!item.note.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(text_tokens::BODY_SM)
                        .text_color(theme.muted_foreground)
                        .child(item.note.clone()),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(SPACE_SM)
                    .flex_wrap()
                    .child(meta)
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap(SPACE_XS)
                            .child(edit_btn)
                            .child(archive_btn),
                    ),
            ),
    )
    .into_any_element()
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let pending = app.pending_checkin.clone();
    let today = model::today();

    // MARK: 数据（与 Phase 2 完全一致的分组口径）

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

    let total_rate = if active_today.is_empty() {
        0
    } else {
        done_today.len() * 100 / active_today.len()
    };

    // MARK: 完成率 progress 卡
    let progress_card = card_with_theme(
        &theme,
        CardVariant::Default,
        None::<SharedString>,
        None::<SharedString>,
        div()
            .flex()
            .items_center()
            .gap(SPACE_MD)
            .child(
                div()
                    .text_size(text_tokens::BODY_SM)
                    .text_color(theme.muted_foreground)
                    .child("今日完成率"),
            )
            .child(
                div()
                    .text_size(text_tokens::BODY_EMPH)
                    .font_weight(FontWeight(600.))
                    .text_color(theme.foreground)
                    .child(format!("{total_rate}%")),
            )
            .child(div().flex_1().min_w_0().child(progress(
                &theme,
                total_rate as f32 / 100.0,
                theme.success,
                px(10.),
            )))
            .child(
                div()
                    .text_size(text_tokens::BODY_SM)
                    .font_weight(FontWeight(600.))
                    .text_color(theme.foreground)
                    .child(format!("{}/{}", done_today.len(), active_today.len())),
            ),
    );

    // MARK: 3 列看板
    let mut board = div().flex().gap(SPACE_MD).flex_1().min_h_0();

    for (title, which, col_key) in COLUMNS {
        let items: Vec<CheckIn> = match which {
            0 => pending_today.clone(),
            1 => done_today.clone(),
            _ => all.clone(),
        };
        let accent = match which {
            0 => theme.warning,
            1 => theme.success,
            _ => theme.accent,
        };

        let mut column = div()
            .flex()
            .flex_col()
            .gap(SPACE_SM)
            .p(SPACE_MD)
            .rounded(RADIUS_LG)
            .border_1()
            .border_color(theme.border)
            .bg(panel(&theme))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap(SPACE_SM)
                    .child(
                        div()
                            .text_size(text_tokens::BODY_SM)
                            .font_weight(FontWeight(600.))
                            .text_color(theme.foreground)
                            .child(title),
                    )
                    .child(badge(items.len(), accent)),
            );

        if items.is_empty() {
            column = column.child(empty_state(
                &theme,
                EmptyVariant::Compact,
                IconName::CircleCheckBig,
                "暂无任务",
                None::<SharedString>,
                None::<AnyElement>,
            ));
        }

        for item in &items {
            column = column.child(task_card(&theme, &this, item, today, col_key));
        }

        board = board.child(
            div()
                .flex_1()
                .min_w_0()
                .overflow_y_scrollbar()
                .child(column),
        );
    }

    // MARK: 已归档 chips（恢复）
    let archived: Vec<CheckIn> = app
        .db
        .live_checkins()
        .filter(|c| c.archived)
        .cloned()
        .collect();
    let mut archive_row = div().flex().items_center().gap(SPACE_MD).flex_wrap();
    for c in &archived {
        let id = c.id.clone();
        let this_unarchive = this.clone();
        archive_row = archive_row.child(
            div()
                .flex()
                .items_center()
                .gap(SPACE_XS)
                .child(chip(&theme, c.title.clone(), theme.muted_foreground))
                .child(button::link(
                    eid("checkin-restore", &id),
                    "恢复",
                    move |_e, window, cx| {
                        let id = id.clone();
                        this_unarchive.update(cx, |this, cx| {
                            this.db.set_checkin_archived(&id, false);
                            this.commit(window, cx);
                        });
                    },
                )),
        );
    }
    let archive_section = div()
        .flex()
        .flex_col()
        .gap(SPACE_SM)
        .child(
            div()
                .text_size(text_tokens::CAPTION)
                .text_color(theme.muted_foreground)
                .child(format!("已归档 {}", archived.len())),
        )
        .child(archive_row);

    // MARK: 归档二次确认（§3.12 alert_dialog，唯一 id + 真取消）
    let archive_target = ARCHIVE_CONFIRM.with(|c| c.borrow().clone());
    let confirm_overlay: Option<AnyElement> = archive_target.map(|id| {
        let title = app
            .db
            .live_checkins()
            .find(|c| c.id == id)
            .map(|c| c.title.clone())
            .unwrap_or_default();
        let prefix = format!("checkin-arch:{id}");
        let this_ok = this.clone();
        let this_cancel = this.clone();
        let dialog = alert_dialog(
            &theme,
            &prefix,
            format!("归档「{title}」？"),
            "不再出现在清单，可恢复。",
            "归档",
            "取消",
            AlertVariant::Primary,
            move |_e, window, cx| {
                let id = id.clone();
                ARCHIVE_CONFIRM.with(|c| *c.borrow_mut() = None);
                this_ok.update(cx, |this, cx| {
                    this.db.set_checkin_archived(&id, true);
                    this.commit(window, cx);
                });
            },
            move |_e, _w, cx| {
                ARCHIVE_CONFIRM.with(|c| *c.borrow_mut() = None);
                this_cancel.update(cx, |_this, cx| cx.notify());
            },
        );
        overlay(&theme, dialog).into_any_element()
    });

    // MARK: 庆祝瞬间
    let celebration = CELEBRATION.with(|c| c.get());
    let celebration_overlay: Option<AnyElement> =
        (celebration != 0).then(|| celebration_layer(&theme, celebration));

    div()
        .relative()
        .flex()
        .flex_col()
        .size_full()
        .p(SPACE_2XL)
        .gap(SPACE_XL)
        .child(section_title(&theme, "今日打卡", None::<AnyElement>))
        .child(progress_card)
        .child(board)
        .when(!archived.is_empty(), |d| d.child(archive_section))
        .when_some(celebration_overlay, |d, layer| d.child(layer))
        .when_some(pending, |d, editor| d.child(overlay(&theme, editor)))
        .when_some(confirm_overlay, |d, confirm| d.child(confirm))
}
