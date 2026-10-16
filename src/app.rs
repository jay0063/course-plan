use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::notification::Notification;
use gpui::component::scroll::ScrollableElement;
use gpui::component::{ThemeMode, WindowExt};
use gpui::*;

use crate::model::{self, Database, Repeat};
use crate::store;
use crate::ui::widgets::{Skin, eid};
use crate::views;
use crate::views::checkins::{CheckInDraft, CheckInEditor};
use crate::views::courses::{CourseDraft, CourseEditor, CourseFilter};
use crate::views::events::{EventDraft, EventEditor};
use crate::views::memos::{MemoDraft, MemoEditor, MemoFilter};
use crate::views::reminders::{ReminderDraft, ReminderEditor};
use crate::views::schedule::SessionDraft;
use crate::views::schedule::SessionEditor;
use crate::views::settings::ProfileInputs;

pub const APP_ID: &str = "com.courseplan.app";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Dashboard,
    Courses,
    Schedule,
    Events,
    Reminders,
    CheckIns,
    Memos,
    Settings,
}

impl Page {
    pub fn label(self) -> &'static str {
        match self {
            Page::Dashboard => "今日概览",
            Page::Courses => "课程管理",
            Page::Schedule => "计划表",
            Page::Events => "日程安排",
            Page::Reminders => "提醒事项",
            Page::CheckIns => "打卡任务",
            Page::Memos => "备忘笔记",
            Page::Settings => "设置",
        }
    }

    pub fn icon(self) -> IconName {
        match self {
            Page::Dashboard => IconName::LayoutDashboard,
            Page::Courses => IconName::BookOpen,
            Page::Schedule => IconName::CalendarDays,
            Page::Events => IconName::Clock,
            Page::Reminders => IconName::Bell,
            Page::CheckIns => IconName::ListTodo,
            Page::Memos => IconName::NotebookPen,
            Page::Settings => IconName::Settings,
        }
    }

    pub const ALL: [Page; 8] = [
        Page::Dashboard,
        Page::Courses,
        Page::Schedule,
        Page::Events,
        Page::Reminders,
        Page::CheckIns,
        Page::Memos,
        Page::Settings,
    ];
}

pub struct App {
    pub db: Database,
    pub page: Page,
    pub course_filter: CourseFilter,
    pub memo_filter: MemoFilter,
    pub selected_date: chrono::NaiveDate,
    pub show_done_reminders: bool,
    pub pending_course: Option<Entity<CourseEditor>>,
    pub pending_session: Option<Entity<SessionEditor>>,
    pub pending_event: Option<Entity<EventEditor>>,
    pub pending_reminder: Option<Entity<ReminderEditor>>,
    pub pending_checkin: Option<Entity<CheckInEditor>>,
    pub pending_memo: Option<Entity<MemoEditor>>,
    pub profile_inputs: Option<Entity<ProfileInputs>>,
    _timer: Option<Task<()>>,
}

impl App {
    pub fn new() -> Self {
        Self::with_database(store::load())
    }

    /// 使用注入的 [`Database`] 构造 App（不读写磁盘）。
    ///
    /// 逻辑与 [`App::new`] 完全一致，只是数据来源改为调用方提供；供集成测试 /
    /// 无头渲染测试注入确定性数据（如 `store::sample_data()`）。
    pub fn with_database(mut db: Database) -> Self {
        let page = if db.profile.auto_load_today {
            Page::Dashboard
        } else {
            Page::Courses
        };
        close_expired_once(&mut db);
        Self {
            db,
            page,
            course_filter: CourseFilter::All,
            memo_filter: MemoFilter::All,
            selected_date: model::today(),
            show_done_reminders: false,
            pending_course: None,
            pending_session: None,
            pending_event: None,
            pending_reminder: None,
            pending_checkin: None,
            pending_memo: None,
            profile_inputs: None,
            _timer: None,
        }
    }

    /// 写入数据并刷新界面。
    pub fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = store::save(&self.db) {
            window.push_notification(Notification::error(err).title("保存失败"), cx);
        }
        cx.notify();
    }

    pub fn goto(&mut self, page: Page, cx: &mut Context<Self>) {
        self.page = page;
        cx.notify();
    }

    // MARK: - 编辑器

    pub fn open_course_editor(
        &mut self,
        draft: CourseDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let me = cx.entity();
        let editor = CourseEditor::new(
            &draft,
            window,
            cx,
            move |draft: CourseDraft, window, app| {
                me.update(app, |this, cx| {
                    if draft.name.trim().is_empty() {
                        window.push_notification(
                            Notification::error("课程名称不能为空").title("无法保存"),
                            cx,
                        );
                        return;
                    }
                    views::courses::apply_draft(&mut this.db, &draft);
                    this.pending_course = None;
                    this.commit(window, cx);
                });
            },
        );
        self.pending_course = Some(editor);
        cx.notify();
    }

    pub fn open_session_editor(
        &mut self,
        draft: SessionDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let me = cx.entity();
        let courses: Vec<(String, String)> = self
            .db
            .courses
            .iter()
            .map(|c| (c.id.clone(), c.name.clone()))
            .collect();
        let editor = SessionEditor::new(
            &draft,
            courses,
            window,
            cx,
            move |draft: SessionDraft, window, app| {
                me.update(app, |this, cx| {
                    if draft.course_id.is_empty() {
                        window.push_notification(
                            Notification::error("请选择课程").title("无法保存"),
                            cx,
                        );
                        return;
                    }
                    views::schedule::apply_draft(&mut this.db, &draft);
                    this.pending_session = None;
                    this.commit(window, cx);
                });
            },
        );
        self.pending_session = Some(editor);
        cx.notify();
    }

    pub fn open_event_editor(
        &mut self,
        draft: EventDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let me = cx.entity();
        let editor = EventEditor::new(&draft, window, cx, move |draft: EventDraft, window, app| {
            me.update(app, |this, cx| {
                if draft.title.trim().is_empty() {
                    window.push_notification(
                        Notification::error("标题不能为空").title("无法保存"),
                        cx,
                    );
                    return;
                }
                views::events::apply_draft(&mut this.db, &draft);
                this.pending_event = None;
                this.commit(window, cx);
            });
        });
        self.pending_event = Some(editor);
        cx.notify();
    }

    pub fn open_reminder_editor(
        &mut self,
        draft: ReminderDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let me = cx.entity();
        let editor = ReminderEditor::new(
            &draft,
            window,
            cx,
            move |draft: ReminderDraft, window, app| {
                me.update(app, |this, cx| {
                    if draft.title.trim().is_empty() {
                        window.push_notification(
                            Notification::error("提醒内容不能为空").title("无法保存"),
                            cx,
                        );
                        return;
                    }
                    views::reminders::apply_draft(&mut this.db, &draft);
                    this.pending_reminder = None;
                    this.commit(window, cx);
                });
            },
        );
        self.pending_reminder = Some(editor);
        cx.notify();
    }

    pub fn open_checkin_editor(
        &mut self,
        draft: CheckInDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let me = cx.entity();
        let editor = CheckInEditor::new(
            &draft,
            window,
            cx,
            move |draft: CheckInDraft, window, app| {
                me.update(app, |this, cx| {
                    if draft.title.trim().is_empty() {
                        window.push_notification(
                            Notification::error("任务名称不能为空").title("无法保存"),
                            cx,
                        );
                        return;
                    }
                    views::checkins::apply_draft(&mut this.db, &draft);
                    this.pending_checkin = None;
                    this.commit(window, cx);
                });
            },
        );
        self.pending_checkin = Some(editor);
        cx.notify();
    }

    pub fn open_memo_editor(
        &mut self,
        draft: MemoDraft,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let me = cx.entity();
        let editor = MemoEditor::new(&draft, window, cx, move |draft: MemoDraft, window, app| {
            me.update(app, |this, cx| {
                views::memos::apply_draft(&mut this.db, &draft);
                this.pending_memo = None;
                this.commit(window, cx);
            });
        });
        self.pending_memo = Some(editor);
        cx.notify();
    }

    /// “快速添加”按钮：根据当前页面打开对应编辑器。
    pub fn quick_add(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.page {
            Page::Schedule => self.open_session_editor(SessionDraft::empty(), window, cx),
            Page::Events => self.open_event_editor(EventDraft::empty(), window, cx),
            Page::Reminders => self.open_reminder_editor(ReminderDraft::empty(), window, cx),
            Page::CheckIns => self.open_checkin_editor(CheckInDraft::empty(), window, cx),
            Page::Memos => self.open_memo_editor(MemoDraft::empty(), window, cx),
            _ => self.open_course_editor(CourseDraft::empty(), window, cx),
        }
    }

    // MARK: - 提醒循环

    fn start_timer(&mut self, cx: &mut Context<Self>) {
        if self._timer.is_some() {
            return;
        }
        let me = cx.entity();
        self._timer = Some(cx.spawn(async move |_this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(30))
                    .await;
                let Some(window) = cx.update(|cx| cx.active_window()) else {
                    continue;
                };
                let today = model::today();
                let stamp = model::now_string();
                let day = today.format("%Y-%m-%d").to_string();

                me.update(cx, |this, cx| {
                    if !this.db.profile.reminder_enabled {
                        return;
                    }
                    let due: Vec<(String, String, String)> = this
                        .db
                        .reminders
                        .iter()
                        .filter(|r| !r.done)
                        .filter(|r| r.notified_for != day)
                        .filter(|r| is_due(r.due_at.trim(), &stamp))
                        .map(|r| {
                            (
                                r.title.clone(),
                                r.content.clone(),
                                r.repeat.label().to_string(),
                            )
                        })
                        .collect();

                    if due.is_empty() {
                        return;
                    }
                    for (title, content, repeat) in due {
                        let note = Notification::warning(format!("{title} · {content}"))
                            .title(format!("课程提醒（{repeat}）"))
                            .in_app_and_system();
                        let _ =
                            window.update(cx, |_, window, cx| window.push_notification(note, cx));
                    }
                    for r in this.db.reminders.iter_mut() {
                        if !r.done {
                            r.notified_for = day.clone();
                        }
                    }
                    let _ = store::save(&this.db);
                    cx.notify();
                });
            }
        }));
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}

/// 判断 `due_at`（形如 "2026-03-05 20:00"）是否已到期。
pub fn is_due(due_at: &str, now: &str) -> bool {
    match (parse_stamp(due_at), parse_stamp(now)) {
        (Some(due), Some(now)) => due <= now,
        _ => false,
    }
}

fn parse_stamp(text: &str) -> Option<chrono::NaiveDateTime> {
    let text = text.trim();
    for fmt in ["%Y-%m-%d %H:%M", "%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M"] {
        if let Ok(v) = chrono::NaiveDateTime::parse_from_str(text, fmt) {
            return Some(v);
        }
    }
    chrono::NaiveDate::parse_from_str(text, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
}

/// 今天到期且未完成的提醒数量，用于侧边栏徽标。
pub fn pending_reminders(db: &Database) -> usize {
    let stamp = model::now_string();
    db.reminders
        .iter()
        .filter(|r| !r.done && is_due(r.due_at.trim(), &stamp))
        .count()
}

/// 把已经过期的一次性提醒标记为完成。
pub fn close_expired_once(db: &mut Database) {
    let today = model::today();
    for r in db.reminders.iter_mut() {
        if r.repeat == Repeat::Once
            && !r.done
            && chrono::NaiveDate::parse_from_str(r.due_at.get(..10).unwrap_or(""), "%Y-%m-%d")
                .is_ok_and(|d| d < today)
        {
            r.done = true;
        }
    }
}

impl Render for App {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.start_timer(cx);

        let skin = Skin::new(cx.theme());

        let page = self.page;
        let content: AnyElement = match page {
            Page::Dashboard => views::dashboard::render(self, cx).into_any_element(),
            Page::Courses => views::courses::render(self, cx).into_any_element(),
            Page::Schedule => views::schedule::render(self, cx).into_any_element(),
            Page::Events => views::events::render(self, cx).into_any_element(),
            Page::Reminders => views::reminders::render(self, cx).into_any_element(),
            Page::CheckIns => views::checkins::render(self, cx).into_any_element(),
            Page::Memos => views::memos::render(self, cx).into_any_element(),
            Page::Settings => views::settings::render(self, window, cx).into_any_element(),
        }
        .into_any_element();

        let sidebar = render_sidebar(self, &skin, cx);
        let topbar = render_topbar(self, &skin, cx);

        div()
            .flex()
            .size_full()
            .bg(skin.surface)
            .text_color(skin.fg)
            .relative()
            .child(sidebar)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(topbar)
                    .child(div().flex_1().min_h_0().overflow_hidden().child(content)),
            )
    }
}

fn render_sidebar(app: &App, skin: &Skin, cx: &mut Context<App>) -> AnyElement {
    let (fg, muted, primary, primary_fg, border) = (
        skin.fg,
        skin.muted,
        skin.primary,
        skin.primary_fg,
        skin.border,
    );
    let bg = skin.sidebar;
    let this = cx.entity();
    let page = app.page;
    let name = if app.db.profile.student.is_empty() {
        "课程计划".to_string()
    } else {
        format!("{}的课程", app.db.profile.student)
    };
    let grade = app.db.profile.grade.clone();
    let today_label = model::fmt_date_cn(model::today());
    let today = model::today();

    let mut items = div().flex().flex_col().gap(px(2.)).p_2();
    for p in Page::ALL {
        let active = p == page;
        let this = this.clone();
        let badge = match p {
            Page::Reminders => pending_reminders(&app.db),
            Page::CheckIns => app
                .db
                .checkins_on(today)
                .into_iter()
                .filter(|c| !c.done_on(today))
                .count(),
            _ => 0,
        };

        items = items.child(
            div()
                .id(eid("nav", p.label()))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .h(px(34.))
                .px_3()
                .rounded_md()
                .cursor_pointer()
                .bg(if active {
                    primary.with_alpha(0.14)
                } else {
                    Hsla { a: 0.0, ..fg }
                })
                .hover(|s| {
                    s.bg(if active {
                        primary.with_alpha(0.20)
                    } else {
                        Hsla { a: 0.06, ..fg }
                    })
                })
                .on_click(move |_e, _w, cx| {
                    this.update(cx, |this, cx| this.goto(p, cx));
                })
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Icon::new(p.icon())
                                .with_size(Size::Small)
                                .text_color(if active { primary } else { muted }),
                        )
                        .child(
                            div()
                                .text_sm()
                                .font_weight(if active {
                                    FontWeight(600.0)
                                } else {
                                    FontWeight(400.0)
                                })
                                .text_color(if active { fg } else { muted })
                                .child(p.label()),
                        ),
                )
                .when(badge > 0, |d| {
                    d.child(
                        div()
                            .min_w(px(18.))
                            .h(px(18.))
                            .px_1()
                            .rounded_full()
                            .bg(primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(
                                div()
                                    .text_xs()
                                    .font_semibold()
                                    .text_color(primary_fg)
                                    .child(badge.to_string()),
                            ),
                    )
                }),
        );
    }

    div()
        .flex()
        .flex_col()
        .h_full()
        .w(px(216.))
        .flex_shrink_0()
        .bg(bg)
        .border_r_1()
        .border_color(border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_4()
                .py_4()
                .child(
                    div()
                        .size(px(30.))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .justify_center()
                        .bg(primary)
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(primary_fg)
                                .child("课"),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .child(div().text_sm().font_semibold().text_color(fg).child(name))
                        .when(!grade.is_empty(), |d| {
                            d.child(div().text_xs().text_color(muted).child(grade.clone()))
                        }),
                ),
        )
        .child(div().flex_1().overflow_y_scrollbar().child(items))
        .child(
            div()
                .px_4()
                .py_3()
                .border_t_1()
                .border_color(border)
                .text_xs()
                .text_color(muted)
                .child(today_label),
        )
        .into_any_element()
}

fn render_topbar(app: &App, skin: &Skin, cx: &mut Context<App>) -> AnyElement {
    let (fg, muted, border) = (skin.fg, skin.muted, skin.border);
    let this = cx.entity();
    let page = app.page;
    let dark = cx.theme().is_dark();

    div()
        .flex()
        .items_center()
        .justify_between()
        .gap_3()
        .h(px(52.))
        .px_5()
        .flex_shrink_0()
        .bg(skin.panel)
        .border_b_1()
        .border_color(border)
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Icon::new(page.icon())
                        .with_size(Size::Small)
                        .text_color(muted),
                )
                .child(
                    div()
                        .text_base()
                        .font_semibold()
                        .text_color(fg)
                        .child(page.label()),
                ),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    Button::new("toggle-theme")
                        .ghost()
                        .compact()
                        .icon(Icon::new(if dark { IconName::Sun } else { IconName::Moon }))
                        .tooltip(if dark {
                            "切换到浅色主题"
                        } else {
                            "切换到深色主题"
                        })
                        .on_click(move |_e, window, app| {
                            let mode = if app.theme().is_dark() {
                                ThemeMode::Light
                            } else {
                                ThemeMode::Dark
                            };
                            Theme::change(mode, Some(window), app);
                        }),
                )
                .child(
                    Button::new("quick-add")
                        .primary()
                        .compact()
                        .icon(Icon::new(IconName::Plus))
                        .label("快速添加")
                        .on_click(move |_e, window, cx| {
                            this.update(cx, |this, cx| this.quick_add(window, cx));
                        }),
                ),
        )
        .into_any_element()
}
