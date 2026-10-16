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
use crate::model::{Course, CourseKind, Database, WEEKDAY_LABELS};
use crate::ui::form::field_row;
use crate::ui::select::{Select, SelectOption};
use crate::ui::widgets::{
    PALETTE, action_button, chip, close_editor, eid, empty_state, palette_at, panel, section_title,
};

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

/// 课程编辑面板。作为独立视图渲染，由 App 以遮罩方式承载。
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
                    .label("课程类型")
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

        let save_entity = this.clone();
        let this_close = this.clone();

        div()
            .w(px(520.))
            .max_h(px(680.))
            .overflow_y_scrollbar()
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
                    .child(title),
            )
            .child(field_row("课程名称", Input::new(&name).h(px(32.)).w_full()))
            .child(kind)
            .child(field_row("科目", Input::new(&subject).h(px(32.)).w_full()))
            .child(field_row(
                "老师/机构",
                Input::new(&teacher).h(px(32.)).w_full(),
            ))
            .child(field_row(
                "上课地点",
                Input::new(&location).h(px(32.)).w_full(),
            ))
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
                    .font_medium()
                    .text_color(theme.muted_foreground)
                    .child("标识颜色"),
            )
            .child(color_row(&theme, self.color, this.clone()))
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .pt_2()
                    .child(
                        Button::new("course-cancel")
                            .secondary()
                            .compact()
                            .label("取消")
                            .on_click(move |_e, _window, app| {
                                close_editor(this_close.read(app).owner.clone(), app);
                            }),
                    )
                    .child(
                        Button::new("course-save")
                            .primary()
                            .compact()
                            .label("保存")
                            .on_click(move |_e, window, app| {
                                let (draft, cb) = {
                                    let guard = save_entity.read(app);
                                    (guard.snapshot(app), guard.on_save.clone())
                                };
                                cb(draft, window, app);
                            }),
                    ),
            )
    }
}

fn color_row(theme: &Theme, selected: usize, this: Entity<CourseEditor>) -> impl IntoElement {
    let mut row = div().flex().gap_2().flex_wrap();
    for i in 0..PALETTE.len() {
        let active = i == selected;
        let this = this.clone();
        row = row.child(
            div()
                .id(eid("course-color", &i.to_string()))
                .size(px(26.))
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
    row
}

pub fn render(app: &mut App, cx: &mut Context<App>) -> impl IntoElement {
    let theme = cx.theme().clone();
    let this = cx.entity();
    let filter = app.course_filter;
    let pending = app.pending_course.clone();

    let mut courses: Vec<Course> = app.db.live_courses().cloned().collect();
    match filter {
        CourseFilter::All => {}
        CourseFilter::School => courses.retain(|c| c.kind == CourseKind::School),
        CourseFilter::Tutoring => courses.retain(|c| c.kind == CourseKind::Tutoring),
    }
    courses.sort_by(|a, b| a.name.cmp(&b.name));

    let mut list = div().flex().flex_col().gap_2();
    if courses.is_empty() {
        list = list.child(empty_state(
            &theme,
            IconName::BookOpen,
            "还没有课程",
            Some("点击右上角“新建课程”开始录入"),
        ));
    }

    for course in &courses {
        let color = palette_at(course.color);
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

        let id = course.id.clone();
        let id_b = course.id.clone();
        let this_edit = this.clone();
        let this_del = this.clone();
        let name = course.name.clone();
        let teacher = course.teacher.clone();
        let location = course.location.clone();
        let note = course.note.clone();
        let subject = course.subject.clone();
        let active = course.active;

        let detail = div()
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
                            .font_semibold()
                            .text_color(if active {
                                theme.foreground
                            } else {
                                theme.muted_foreground
                            })
                            .child(name.clone()),
                    )
                    .child(chip(course.kind.label(), kind_color, false))
                    .when(!active, |d| {
                        d.child(chip("已归档", theme.muted_foreground, false))
                    }),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .flex_wrap()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .when(!subject.is_empty(), |d| d.child(format!("科目 {subject}")))
                    .when(!teacher.is_empty(), |d| d.child(teacher.clone()))
                    .when(!location.is_empty(), |d| d.child(format!("@ {location}"))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(theme.muted_foreground)
                    .child(format!("{schedule_text} · 每周 {weekly} 分钟")),
            )
            .when(!note.is_empty(), |d| {
                d.child(
                    div()
                        .text_xs()
                        .text_color(theme.muted_foreground)
                        .child(note.clone()),
                )
            });

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
                        .h(px(44.))
                        .flex_shrink_0()
                        .rounded_full()
                        .bg(color),
                )
                .child(detail)
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_1()
                        .flex_shrink_0()
                        .child(action_button(
                            eid("course-edit", &course.id),
                            "编辑",
                            move |_e, window, cx| {
                                this_edit.update(cx, |this, cx| {
                                    if let Some(c) = this.db.course(&id).cloned() {
                                        this.open_course_editor(CourseDraft::from(&c), window, cx);
                                    }
                                });
                            },
                        ))
                        .child(action_button(
                            eid("course-del", &course.id),
                            "删除",
                            move |_e, window, cx| {
                                let owner = this_del.clone();
                                confirm_delete_course(owner, window, cx, id_b.clone());
                            },
                        )),
                ),
        );
    }

    let this_add = this.clone();
    let header_action: AnyElement = Button::new("add-course")
        .primary()
        .compact()
        .icon(Icon::new(IconName::Plus))
        .label("新建课程")
        .on_click(move |_e, window, cx| {
            this_add.update(cx, |this, cx| {
                let draft = CourseDraft {
                    id: String::new(),
                    name: String::new(),
                    kind: CourseKind::School,
                    subject: String::new(),
                    teacher: String::new(),
                    location: String::new(),
                    note: String::new(),
                    color: 0,
                };
                this.open_course_editor(draft, window, cx);
            });
        })
        .into_any_element();

    let mut filter_bar = div().flex().items_center().gap_2();
    for (label, f) in [
        ("全部", CourseFilter::All),
        ("学校课程", CourseFilter::School),
        ("补习课程", CourseFilter::Tutoring),
    ] {
        filter_bar = filter_bar.child(filter_chip(&theme, label, filter == f, this.clone(), f));
    }

    div()
        .flex()
        .flex_col()
        .size_full()
        .p_5()
        .gap_4()
        .child(section_title(&theme, "课程管理", Some(header_action)))
        .child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_3()
                .child(filter_bar)
                .child(
                    div()
                        .text_sm()
                        .text_color(theme.muted_foreground)
                        .child(format!("共 {} 门", courses.len())),
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

fn filter_chip(
    theme: &Theme,
    label: &'static str,
    active: bool,
    this: Entity<App>,
    filter: CourseFilter,
) -> impl IntoElement {
    div()
        .id(eid("course-filter", label))
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
        .hover(|s| s.border_color(theme.primary_hover))
        .id(eid("course-filter", label))
        .on_click(move |_e, _w, cx| {
            this.update(cx, |this, cx| {
                this.course_filter = filter;
                cx.notify();
            });
        })
        .child(label.to_string())
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

fn confirm_delete_course(owner: Entity<App>, window: &mut Window, cx: &mut gpui::App, id: String) {
    let name = owner
        .read(cx)
        .db
        .course(&id)
        .map(|c| c.name.clone())
        .unwrap_or_default();
    let owner = owner.clone();
    window.open_alert_dialog(cx, move |alert, _w, _cx| {
        let owner = owner.clone();
        let id = id.clone();
        alert
            .title("删除课程")
            .description(format!("确定删除「{name}」吗？相关课表安排也会一并移除。"))
            .ok_text("删除")
            .ok_variant(ButtonVariant::Danger)
            .on_ok(move |_e, window, cx| {
                owner.update(cx, |this, cx| {
                    this.db.delete_course(&id);
                    this.commit(window, cx);
                });
                true
            })
    });
}
