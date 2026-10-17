//! 课程管理（DESIGN.md §4.2）。
//!
//! 页面名 + 顶部主 CTA 归顶栏（§3.1.4）；本页内容区只有 `section_title` 分节、
//! `tab_chips` 过滤与课程卡片列表。

use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::input::{Input, InputState};
use gpui::component::input::{Textarea, TextareaState};
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use std::rc::Rc;

use crate::app::{App, Page};
use crate::model::{Course, CourseKind, Database, WEEKDAY_LABELS};
use crate::ui::components::button;
use crate::ui::components::card::{CardVariant, card_with_theme};
use crate::ui::components::empty_state::{EmptyVariant, empty_state};
use crate::ui::components::overlay::{AlertVariant, alert_dialog, dialog, overlay, section_title};
use crate::ui::components::tab_chips::tab_chips;
use crate::ui::components::tag::{chip, tag};
use crate::ui::form::field_row;
use crate::ui::palette::palette_at_for;
use crate::ui::select::{Select, SelectOption};
use crate::ui::tokens::text;
use crate::ui::tokens::{INPUT_H, RADIUS_FULL, RADIUS_MD, SPACE_MD, SPACE_SM, SPACE_XL, SPACE_XS};
use crate::ui::widgets::{close_editor, eid};

// MARK: - 删除确认挂起态（courses / schedule / events 三页共用）

/// 删除对象类别。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DeleteKind {
    Course,
    Session,
    Event,
}

impl DeleteKind {
    fn slug(self) -> &'static str {
        match self {
            DeleteKind::Course => "course",
            DeleteKind::Session => "session",
            DeleteKind::Event => "event",
        }
    }

    fn title(self) -> &'static str {
        match self {
            DeleteKind::Course => "删除课程",
            DeleteKind::Session => "删除安排",
            DeleteKind::Event => "删除日程",
        }
    }

    fn description(self, name: &str) -> SharedString {
        match self {
            DeleteKind::Course => {
                format!("确定删除「{name}」吗？相关课表安排也会一并移除。").into()
            }
            DeleteKind::Session => "确定从课表中删除这条安排吗？".into(),
            DeleteKind::Event => "确定删除这条日程吗？".into(),
        }
    }

    fn remove(self, db: &mut Database, id: &str) {
        match self {
            DeleteKind::Course => db.delete_course(id),
            DeleteKind::Session => db.delete_session(id),
            DeleteKind::Event => db.delete_event(id),
        }
    }
}

/// 一条待确认的删除。
#[derive(Clone)]
pub(crate) struct DeleteRequest {
    pub page: Page,
    pub kind: DeleteKind,
    pub id: String,
    pub name: String,
}

/// 视图层的删除确认挂起态。
///
/// 设计系统要求删除必须经 `ui::components::overlay::alert_dialog` 二次确认，且按钮
/// id 必须唯一（§3.12：`{prefix}:ok` / `{prefix}:cancel`）。三个页面都是无状态渲染
/// 函数，`App`（本泳道不可改）也没有承载它的槽位，因此用 GPUI Global 承载这一份极小
/// 的状态。任一页面渲染时若发现挂起态属于别的页面就丢弃，避免「离开页面后确认框
/// 阴魂不散」。
pub(crate) struct PendingDelete(pub(crate) Option<DeleteRequest>);

impl Global for PendingDelete {}

/// 渲染前丢弃不属于当前页面的挂起确认。
pub(crate) fn discard_stale_delete(cx: &mut Context<App>, page: Page) {
    let stale = cx
        .try_global::<PendingDelete>()
        .and_then(|p| p.0.as_ref())
        .is_some_and(|req| req.page != page);
    if stale {
        cx.set_global(PendingDelete(None));
    }
}

/// 当前挂起的删除确认（若有）。
pub(crate) fn pending_delete(cx: &Context<App>) -> Option<DeleteRequest> {
    cx.try_global::<PendingDelete>().and_then(|p| p.0.clone())
}

/// 打开一条删除确认。
pub(crate) fn request_delete(
    cx: &mut Context<App>,
    page: Page,
    kind: DeleteKind,
    id: String,
    name: String,
) {
    cx.set_global(PendingDelete(Some(DeleteRequest {
        page,
        kind,
        id,
        name,
    })));
    cx.notify();
}

// 遮罩点击拦截由 `ui::components::overlay::overlay` 组件内建
// （根节点 `occlude()`，`BlockMouse` 在 hit-test 走到遮罩时停止）——
// 各视图不再需要自己的 `overlay_click_guard()`。

/// 删除确认对话框（`overlay` + `alert_dialog`：全屏遮罩 + 唯一按钮 id）。
pub(crate) fn delete_confirm_overlay(
    theme: &Theme,
    req: &DeleteRequest,
    app: Entity<App>,
) -> AnyElement {
    let prefix = format!("delete-{}:{}", req.kind.slug(), req.id);
    let ok_app = app.clone();
    let kind = req.kind;
    let ok_id = req.id.clone();
    let on_ok = move |_e: &ClickEvent, window: &mut Window, cx: &mut gpui::App| {
        cx.set_global(PendingDelete(None));
        ok_app.update(cx, |this, cx| {
            kind.remove(&mut this.db, &ok_id);
            this.commit(window, cx);
        });
    };
    let on_cancel = move |_e: &ClickEvent, _w: &mut Window, cx: &mut gpui::App| {
        cx.set_global(PendingDelete(None));
        app.update(cx, |_this, cx| cx.notify());
    };
    overlay(
        theme,
        alert_dialog(
            theme,
            &prefix,
            req.kind.title(),
            req.kind.description(&req.name),
            "删除",
            "取消",
            AlertVariant::Danger,
            on_ok,
            on_cancel,
        ),
    )
    .into_any_element()
}

// MARK: - 过滤器 / 草稿

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CourseFilter {
    All,
    School,
    Tutoring,
}

/// 编辑器提交回来的纯数据。
#[derive(Clone)]
pub struct CourseDraft {
    pub id: String,
    pub name: String,
    pub kind: CourseKind,
    pub subject: String,
    pub teacher: String,
    pub location: String,
    pub note: String,
    pub color: usize,
}

impl CourseDraft {
    pub fn empty() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            kind: CourseKind::School,
            subject: String::new(),
            teacher: String::new(),
            location: String::new(),
            note: String::new(),
            color: 0,
        }
    }

    pub fn from(course: &Course) -> Self {
        Self {
            id: course.id.clone(),
            name: course.name.clone(),
            kind: course.kind,
            subject: course.subject.clone(),
            teacher: course.teacher.clone(),
            location: course.location.clone(),
            note: course.note.clone(),
            color: course.color,
        }
    }
}

type SaveCb = Rc<dyn Fn(CourseDraft, &mut Window, &mut gpui::App)>;

/// 课程编辑器。作为 `overlay` + `dialog` 内容渲染，由 App 以遮罩方式承载。
pub struct CourseEditor {
    owner: Entity<App>,
    id: String,
    name: Entity<InputState>,
    subject: Entity<InputState>,
    teacher: Entity<InputState>,
    location: Entity<InputState>,
    note: Entity<TextareaState>,
    kind: Entity<Select>,
    color: usize,
    is_new: bool,
    on_save: SaveCb,
}

impl CourseEditor {
    pub fn new(
        draft: &CourseDraft,
        window: &mut Window,
        cx: &mut Context<App>,
        on_save: impl Fn(CourseDraft, &mut Window, &mut gpui::App) + 'static,
    ) -> Entity<Self> {
        let owner = cx.entity();
        let kind_initial = Some(match draft.kind {
            CourseKind::School => 0,
            CourseKind::Tutoring => 1,
        });
        let is_new = draft.id.is_empty();

        let mut builder = Self {
            owner,
            id: draft.id.clone(),
            color: draft.color,
            is_new,
            name: cx.new(|cx| InputState::new(window, cx).placeholder("课程名称，如：数学")),
            subject: cx.new(|cx| InputState::new(window, cx).placeholder("如：数学")),
            teacher: cx.new(|cx| InputState::new(window, cx).placeholder("老师 / 机构")),
            location: cx.new(|cx| InputState::new(window, cx).placeholder("教室 / 地点")),
            note: cx.new(|cx| TextareaState::new(window, cx).placeholder("备注")),
            kind: cx.new(|_| {
                Select::new("course-kind")
                    .options(vec![
                        SelectOption::new("学校课程").hint("校内常规课程"),
                        SelectOption::new("补习课程").hint("课外辅导 / 兴趣班"),
                    ])
                    .selected(kind_initial)
            }),
            on_save: Rc::new(on_save),
        };

        builder.seed(draft, window, cx);
        cx.new(|_| builder)
    }

    fn seed(&mut self, draft: &CourseDraft, window: &mut Window, cx: &mut Context<App>) {
        let mut set = |state: &Entity<InputState>, text: String| {
            state.update(cx, |s, cx| s.set_value(text, window, cx));
        };
        set(&self.name, draft.name.clone());
        set(&self.subject, draft.subject.clone());
        set(&self.teacher, draft.teacher.clone());
        set(&self.location, draft.location.clone());
        self.note.update(cx, |s, cx| {
            s.set_auto_grow(2, 5, cx);
            s.set_value(draft.note.clone(), window, cx);
        });
    }

    fn snapshot(&self, cx: &gpui::App) -> CourseDraft {
        CourseDraft {
            id: self.id.clone(),
            name: self.name.read(cx).value().to_string(),
            kind: match self.kind.read(cx).selected_index() {
                Some(1) => CourseKind::Tutoring,
                _ => CourseKind::School,
            },
            subject: self.subject.read(cx).value().to_string(),
            teacher: self.teacher.read(cx).value().to_string(),
            location: self.location.read(cx).value().to_string(),
            note: self.note.read(cx).value().to_string(),
            color: self.color,
        }
    }
}

impl Render for CourseEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let this = cx.entity();

        let name = self.name.clone();
        let subject = self.subject.clone();
        let teacher = self.teacher.clone();
        let location = self.location.clone();
        let note = self.note.clone();
        let kind = self.kind.clone();
        let title = if self.is_new {
            "新建课程"
        } else {
            "编辑课程"
        };

        let body = div()
            .w_full()
            .flex()
            .flex_col()
            .gap(SPACE_MD)
            .child(field_row("课程名称", Input::new(&name).h(INPUT_H).w_full()))
            .child(field_row("课程类型", div().flex().child(kind)))
            .child(field_row("科目", Input::new(&subject).h(INPUT_H).w_full()))
            .child(field_row(
                "老师/机构",
                Input::new(&teacher).h(INPUT_H).w_full(),
            ))
            .child(field_row(
                "上课地点",
                Input::new(&location).h(INPUT_H).w_full(),
            ))
            .child(field_row(
                "备注",
                div()
                    .w_full()
                    .rounded(RADIUS_MD)
                    .border_1()
                    .border_color(theme.border)
                    .child(Textarea::new(&note).w_full()),
            ))
            .child(field_row(
                "标识颜色",
                color_row(&theme, self.color, this.clone()),
            ));

        let cancel_this = this.clone();
        let save_this = this.clone();
        let footer = div()
            .flex()
            .gap(SPACE_SM)
            .child(button::secondary(
                "course-cancel",
                "取消",
                move |_e, _w, cx| {
                    close_editor(cancel_this.read(cx).owner.clone(), cx);
                },
            ))
            .child(button::primary(
                "course-save",
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

/// 8 色课程色板（§2.6）：必须经 `palette_at_for(index, &Theme)` 取色。
fn color_row(theme: &Theme, selected: usize, this: Entity<CourseEditor>) -> impl IntoElement {
    let mut row = div().flex().gap(SPACE_SM).flex_wrap();
    for i in 0..8usize {
        let active = i == selected;
        let swatch_this = this.clone();
        row = row.child(
            div()
                .id(eid("course-color", &i.to_string()))
                .size(px(26.)) // 一次性像素：色块直径（§1.2 例外 ②）
                .flex_shrink_0()
                .rounded(RADIUS_FULL)
                .bg(palette_at_for(i, theme))
                .border_2()
                .border_color(if active {
                    theme.foreground
                } else {
                    theme.foreground.with_alpha(0.0)
                })
                .cursor_pointer()
                .on_click(move |_e, _w, cx| {
                    swatch_this.update(cx, |this, cx| {
                        this.color = i;
                        cx.notify();
                    });
                }),
        );
    }
    row
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let filter = app.course_filter;
    let pending = app.pending_course.clone();

    discard_stale_delete(cx, Page::Courses);
    let deleting = pending_delete(cx);

    let mut courses: Vec<Course> = app.db.live_courses().cloned().collect();
    match filter {
        CourseFilter::All => {}
        CourseFilter::School => courses.retain(|c| c.kind == CourseKind::School),
        CourseFilter::Tutoring => courses.retain(|c| c.kind == CourseKind::Tutoring),
    }
    courses.sort_by(|a, b| a.name.cmp(&b.name));

    let mut list = div().flex().flex_col().gap(SPACE_MD);
    if courses.is_empty() {
        list = list.child(empty_state(
            &theme,
            EmptyVariant::Illustrated,
            IconName::BookOpen,
            "还没有课程",
            Some("点右上角「快速添加」开始录入"),
            None::<AnyElement>,
        ));
    }

    for course in &courses {
        let color = palette_at_for(course.color, &theme);
        let kind_color = match course.kind {
            CourseKind::School => theme.info,
            CourseKind::Tutoring => theme.warning,
        };
        let sessions = app.db.sessions_of(&course.id);
        let weekly: i64 = sessions
            .iter()
            .map(|s| (s.end_minutes() - s.start_minutes()).max(0) as i64)
            .sum();
        let schedule_text = if sessions.is_empty() {
            "未排课".to_string()
        } else {
            let mut days: Vec<u8> = sessions.iter().map(|s| s.weekday).collect();
            days.sort();
            days.dedup();
            days.iter()
                .map(|d| WEEKDAY_LABELS[(*d as usize).clamp(1, 7) - 1])
                .collect::<Vec<_>>()
                .join("、")
        };

        let id_row = course.id.clone();
        let id_edit = course.id.clone();
        let id_del = course.id.clone();
        let name_title = course.name.clone();
        let name_del = course.name.clone();
        let this_row = this.clone();
        let this_edit = this.clone();
        let this_del = this.clone();
        let active = course.active;

        // 科目 / 老师 / 地点：拼成一行密集信息。
        let mut meta_parts: Vec<String> = Vec::new();
        if !course.subject.is_empty() {
            meta_parts.push(course.subject.clone());
        }
        if !course.teacher.is_empty() {
            meta_parts.push(course.teacher.clone());
        }
        if !course.location.is_empty() {
            meta_parts.push(format!("@ {}", course.location));
        }
        let meta_line = meta_parts.join(" · ");
        let note = course.note.clone();

        let detail = div()
            .id(eid("course-row", &course.id))
            .flex_1()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(SPACE_XS)
            .cursor_pointer()
            .on_click(move |_e, window, cx| {
                this_row.update(cx, |this, cx| {
                    if let Some(c) = this.db.course(&id_row).cloned() {
                        this.open_course_editor(CourseDraft::from(&c), window, cx);
                    }
                });
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap(SPACE_SM)
                    .child(
                        div()
                            .text_size(text::BODY_EMPH)
                            .font_weight(FontWeight(600.0))
                            .text_color(if active {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .child(name_title),
                    )
                    .child(tag(&theme, course.kind.short(), kind_color))
                    .when(!active, |d| {
                        d.child(chip(&theme, "已归档", theme.muted_foreground))
                    }),
            )
            .when(!meta_line.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(text::CAPTION)
                        .text_color(theme.muted_foreground)
                        .child(meta_line),
                )
            })
            .child(
                div()
                    .text_size(text::CAPTION)
                    .text_color(theme.muted_foreground)
                    .child(format!("{schedule_text} · 每周 {weekly} 分钟")),
            )
            .when(!note.is_empty(), |d| {
                d.child(
                    div()
                        .text_size(text::CAPTION)
                        .text_color(theme.muted_foreground)
                        .child(note.clone()),
                )
            });

        let actions = div()
            .flex()
            .items_center()
            .gap(SPACE_XS)
            .flex_shrink_0()
            .child(button::icon_only(
                eid("course-edit", &course.id),
                IconName::Pencil,
                "编辑课程",
                move |_e, window, cx| {
                    this_edit.update(cx, |this, cx| {
                        if let Some(c) = this.db.course(&id_edit).cloned() {
                            this.open_course_editor(CourseDraft::from(&c), window, cx);
                        }
                    });
                },
            ))
            .child(button::icon_only(
                eid("course-del", &course.id),
                IconName::Trash,
                "删除课程",
                move |_e, _window, cx| {
                    this_del.update(cx, |_this, cx| {
                        request_delete(
                            cx,
                            Page::Courses,
                            DeleteKind::Course,
                            id_del.clone(),
                            name_del.clone(),
                        );
                    });
                },
            ));

        list = list.child(card_with_theme(
            &theme,
            CardVariant::Default,
            None::<SharedString>,
            None::<SharedString>,
            div()
                .flex()
                .items_start()
                .gap(SPACE_MD)
                .child(
                    div()
                        .w(px(4.)) // 一次性像素：4px 色条（§1.2 例外 ②）
                        .h(px(44.))
                        .flex_shrink_0()
                        .rounded(RADIUS_FULL)
                        .bg(color),
                )
                .child(detail)
                .child(actions),
        ));
    }

    let filter_this = this.clone();
    let filters = tab_chips(
        &theme,
        "course-filter",
        &[
            ("全部", CourseFilter::All),
            ("校内", CourseFilter::School),
            ("补习", CourseFilter::Tutoring),
        ],
        filter,
        move |value, _w, cx| {
            filter_this.update(cx, |this, cx| {
                this.course_filter = value;
                cx.notify();
            });
        },
    );

    let count: AnyElement = div()
        .text_size(text::LABEL_SM)
        .text_color(theme.muted_foreground)
        .child(format!("{} 门", courses.len()))
        .into_any_element();

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap(SPACE_XL)
        .child(section_title(&theme, "全部课程", Some(count)))
        .child(filters)
        .child(div().flex_1().min_h_0().overflow_y_scrollbar().child(list))
        .when_some(pending, |d, e| d.child(overlay(&theme, e)))
        .when_some(deleting, |d, req| {
            d.child(delete_confirm_overlay(&theme, &req, this.clone()))
        })
}

/// 把编辑器提交的数据写回数据库（经 Mutation API，本地改动自动盖单调戳）。
pub fn apply_draft(db: &mut Database, draft: &CourseDraft) {
    let mut course = match db.courses.iter().find(|c| c.id == draft.id) {
        Some(existing) => existing.clone(),
        None => {
            let mut fresh = Course::new(draft.name.clone(), draft.kind);
            if !draft.id.is_empty() {
                fresh.id = draft.id.clone();
            }
            fresh
        }
    };
    course.name = draft.name.clone();
    course.kind = draft.kind;
    course.subject = draft.subject.clone();
    course.teacher = draft.teacher.clone();
    course.location = draft.location.clone();
    course.note = draft.note.clone();
    course.color = draft.color;
    db.save_course(course);
}
