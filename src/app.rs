use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::button::{Button, ButtonVariants};
use gpui::component::input::InputState;
use gpui::component::notification::Notification;
use gpui::component::scroll::ScrollableElement;
use gpui::component::{ThemeMode, WindowExt};
use gpui::*;

use crate::model::{self, Database, Repeat};
use crate::store;
use crate::sync::{
    FirstLoginPolicy, ProbeSummary, Status, SyncCommand, SyncCoreConfig, SyncEvent, SyncHandle,
};
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

/// 首登对话框数据（收到探针结果、且双方皆非空时展示）。
pub struct FirstLoginPrompt {
    pub round_id: u64,
    pub probe: ProbeSummary,
}

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
    /// 设备本地运行态（提醒「已通知」记账等，不同步），启动时加载一次。
    pub runtime: store::RuntimeState,
    /// 启动时读取数据文件产生的告警（如损坏备份），渲染时提示一次。
    pub startup_warning: Option<String>,
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
    /// 同步 worker 句柄（命令发送 + 事件接收）。
    pub sync: SyncHandle,
    /// 最近一次同步状态（顶栏/设置页显示）。
    pub sync_status: Status,
    /// 设置页同步卡片的输入框（惰性创建）。
    pub sync_server_url: Option<Entity<InputState>>,
    pub sync_username: Option<Entity<InputState>>,
    pub sync_password: Option<Entity<InputState>>,
    /// 同步提示文案（登录结果 / 错误）。
    pub sync_notice: Option<String>,
    /// 首登对话框（双方皆非空时展示）。
    pub first_login_prompt: Option<FirstLoginPrompt>,
    _timer: Option<Task<()>>,
    _sync_consumer: Option<Task<()>>,
}

impl App {
    pub fn new() -> Self {
        let (db, warning) = store::load();
        let mut app = Self::with_database(db);
        app.startup_warning = warning;
        app.runtime = store::load_runtime_state();

        // 读取已保存的同步凭据：有 token 则以后台 worker 恢复同步（§5.2 启动触发）。
        let mut settings = store::load_sync();
        // 设备标识在首次写入 sync.json 时生成；确保持久化，使 worker 与文件一致。
        if settings.device_id.trim().is_empty() {
            let _ = store::save_sync(&mut settings);
        }
        app.db.stamp_floor_ms = settings.push_watermark_ms;
        let has_token = !settings.token.is_empty() && !settings.server_url.is_empty();
        let config = SyncCoreConfig {
            device_id: settings.device_id.clone(),
            server_url: settings.server_url.clone(),
            username: settings.username.clone(),
            user_id: settings.user_id,
            token: settings.token.clone(),
            last_seq: settings.last_seq,
        };
        app.sync = SyncHandle::spawn(config);
        app.sync_status = app.sync.status();
        if has_token {
            app.sync.send(SyncCommand::SyncNow);
        }
        app
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
            runtime: store::RuntimeState::default(),
            startup_warning: None,
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
            sync: SyncHandle::disabled(),
            sync_status: Status::LoggedOut,
            sync_server_url: None,
            sync_username: None,
            sync_password: None,
            sync_notice: None,
            first_login_prompt: None,
            _timer: None,
            _sync_consumer: None,
        }
    }

    /// 写入数据并刷新界面。
    pub fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Err(err) = store::save(&self.db) {
            window.push_notification(Notification::error(err).title("保存失败"), cx);
        }
        self.note_local_change();
        cx.notify();
    }

    /// 本地持久化后的钩子：投递 3 秒防抖的同步触发（§5.2）。
    pub fn note_local_change(&mut self) {
        self.sync.send(SyncCommand::LocalChanged);
    }

    /// 只持久化 data.json（远端 apply 路径；**不**触发 LocalChanged，避免回环）。
    fn persist_data(&mut self, window: gpui::AnyWindowHandle, cx: &mut Context<Self>) {
        if let Err(err) = store::save(&self.db) {
            let _ = window.update(cx, |_, window, cx| {
                window.push_notification(Notification::error(err).title("同步保存失败"), cx);
            });
        }
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
            .live_courses()
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
                    if !draft.id.is_empty() {
                        this.runtime.notified.remove(&draft.id);
                    }
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
                        .live_reminders()
                        .filter(|r| !r.done)
                        .filter(|r| this.runtime.notified.get(&r.id) != Some(&day))
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
                    // 记账移入设备本地运行态（不同步；避免每台设备通知记账互相污染）。
                    for r in this.db.live_reminders().filter(|r| !r.done) {
                        this.runtime.notified.insert(r.id.clone(), day.clone());
                    }
                    let _ = store::save_runtime_state(&this.runtime);
                    cx.notify();
                });
            }
        }));
    }

    // MARK: - 同步接线（UI 线程，单写者）

    /// 启动同步事件消费者（与 `start_timer` 相同的 `cx.spawn` + `WeakEntity` 模式）。
    fn start_sync_consumer(&mut self, cx: &mut Context<Self>) {
        if self._sync_consumer.is_some() {
            return;
        }
        let Some(events) = self.sync.take_events() else {
            return;
        };
        let me = cx.entity();
        self._sync_consumer = Some(cx.spawn(async move |_this, cx| {
            while let Ok(event) = events.recv().await {
                let Some(window) = cx.update(|cx| cx.active_window()) else {
                    continue;
                };
                me.update(cx, |this, cx| this.on_sync_event(event, window, cx));
            }
        }));
    }

    /// 处理一条同步事件（**仅在 UI 线程**）：apply + 持久化 + 回执（§5.1）。
    fn on_sync_event(
        &mut self,
        event: SyncEvent,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        match event {
            SyncEvent::StatusChanged(status) => {
                self.sync_status = status;
            }
            SyncEvent::RoundStarted { .. } => {
                // 轮次起点：重置本轮最小本地改动戳记账（§5.3 R3）。
                self.db.begin_round(0);
            }
            SyncEvent::NeedCandidates { round_id, full } => {
                let since = if full { 0 } else { self.db.stamp_floor_ms };
                let changes = self.db.all_changes(since);
                self.sync
                    .send(SyncCommand::Candidates { round_id, changes });
            }
            SyncEvent::ApplyBatch {
                round_id,
                changes,
                replace_all,
                ..
            } => {
                if replace_all {
                    // 云端覆盖本地：丢弃本地未同步数据（不产生墓碑，§6-3b）。
                    self.db = Database::default();
                }
                for change in &changes {
                    self.db.apply_remote(change);
                }
                // 顺序：先 data.json（durable）→ 再回 Ack（§5.1）。
                self.persist_data(window, cx);
                self.sync.send(SyncCommand::AckRound { round_id });
            }
            SyncEvent::ProbeResult { round_id, probe } => {
                self.handle_probe(round_id, probe);
            }
            SyncEvent::RoundComplete {
                round_id,
                last_seq,
                watermark_base_ms,
            } => {
                self.persist_round_cursors(last_seq, watermark_base_ms, window, cx);
                self.sync.send(SyncCommand::AckRound { round_id });
            }
            SyncEvent::AuthResult {
                ok,
                message,
                server_url,
                user_id,
                username,
                token,
            } => {
                self.handle_auth_result(
                    ok, message, server_url, user_id, username, token, window, cx,
                );
            }
        }
        cx.notify();
    }

    /// 探针结果：服务端空 → 全量推；本地空 → 全量拉；双方非空 → 弹三选一对话框。
    fn handle_probe(&mut self, round_id: u64, probe: ProbeSummary) {
        let server_empty = probe.all_ids.is_empty();
        let local_empty = self.db.all_entity_ids().len() <= 1;
        if server_empty {
            self.sync.send(SyncCommand::FirstLoginPolicy {
                round_id,
                policy: FirstLoginPolicy::Merge,
            });
        } else if local_empty {
            self.sync.send(SyncCommand::FirstLoginPolicy {
                round_id,
                policy: FirstLoginPolicy::CloudWins,
            });
        } else {
            self.first_login_prompt = Some(FirstLoginPrompt { round_id, probe });
        }
    }

    /// 登录成功：按 §3 处理账号切换（归档 + 清空 + 重置游标）并持久化凭据。
    #[allow(clippy::too_many_arguments)] // 事件字段直传，避免额外结构体
    fn handle_auth_result(
        &mut self,
        ok: bool,
        message: String,
        server_url: Option<String>,
        user_id: Option<i64>,
        username: Option<String>,
        token: Option<String>,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        if !ok {
            self.sync_notice = Some(message.clone());
            let _ = window.update(cx, |_, window, cx| {
                window.push_notification(Notification::error(message).title("同步"), cx);
            });
            return;
        }

        let server_url = server_url.unwrap_or_default();
        let mut settings = store::load_sync();
        let switched = settings.server_url != server_url
            || settings.user_id != user_id
            || settings.user_id.is_none();
        let had_previous = settings.user_id.is_some() || !settings.server_url.is_empty();
        if switched && had_previous {
            // 账号切换（§3）：归档当前 data.json，本地 DB 清空为新账号空白空间。
            self.archive_current_data(settings.user_id);
            self.db = Database::default();
            let _ = store::save(&self.db);
        }

        settings.server_url = server_url;
        settings.user_id = user_id;
        settings.username = username.clone().unwrap_or_default();
        settings.token = token.unwrap_or_default();
        // 登录/切换 → 游标与水位重置 → 全量重拉（§3）。
        settings.last_seq = 0;
        settings.push_watermark_ms = 0;
        if let Err(err) = store::save_sync(&mut settings) {
            self.sync_notice = Some(err);
        }
        self.db.stamp_floor_ms = 0;

        let name = username.unwrap_or_default();
        self.sync_notice = Some(format!("已登录：{name}"));
        let _ = window.update(cx, |_, window, cx| {
            window.push_notification(
                Notification::success(format!("已登录 {name}")).title("同步"),
                cx,
            );
        });
    }

    /// 归档当前 data.json 为 `data-<old_user_id>.json`（§3 账号切换）。
    fn archive_current_data(&self, old_user_id: Option<i64>) {
        let path = store::data_file();
        if !path.exists() {
            return;
        }
        let tag = old_user_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let dest = path.with_file_name(format!("data-{tag}.json"));
        let _ = std::fs::rename(&path, &dest);
    }

    /// 整轮完成：写 data.json（批内已写）→ 写 sync.json（游标 + 水位）→ 由调用方 ack。
    fn persist_round_cursors(
        &mut self,
        last_seq: u64,
        watermark_base_ms: i64,
        window: gpui::AnyWindowHandle,
        cx: &mut Context<Self>,
    ) {
        let min_mut = self.db.round_min_mutation_ts();
        let watermark = match min_mut {
            Some(min) => watermark_base_ms.min(min),
            None => watermark_base_ms,
        };
        let mut settings = store::load_sync();
        settings.last_seq = last_seq;
        settings.push_watermark_ms = watermark;
        self.db.stamp_floor_ms = watermark;
        if let Err(err) = store::save_sync(&mut settings) {
            let _ = window.update(cx, |_, window, cx| {
                window.push_notification(Notification::error(err).title("同步保存失败"), cx);
            });
        }
    }

    /// 首登对话框三选一执行（§6-3）。
    pub fn apply_first_login_policy(
        &mut self,
        policy_first_login: FirstLoginPolicyKind,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.first_login_prompt.take() else {
            return;
        };
        let round_id = prompt.round_id;
        let policy = match policy_first_login {
            FirstLoginPolicyKind::Merge => FirstLoginPolicy::Merge,
            FirstLoginPolicyKind::CloudWins => FirstLoginPolicy::CloudWins,
            FirstLoginPolicyKind::LocalWins => {
                // 本地覆盖云端：先重戳（压过服务端已知戳），stamp floor 一并抬高（§6-3c）。
                let base = model::now_ms().max(prompt.probe.server_time_ms.saturating_add(1));
                self.db.bump_all_stamps(base);
                self.db.stamp_floor_ms = base;
                let local_ids = self.db.all_entity_ids();
                FirstLoginPolicy::LocalWins { local_ids }
            }
        };
        if let Err(err) = store::save(&self.db) {
            window.push_notification(Notification::error(err).title("保存失败"), cx);
        }
        self.sync
            .send(SyncCommand::FirstLoginPolicy { round_id, policy });
        cx.notify();
    }

    /// 登出：仅停止同步、保留本地数据（§3）。
    pub fn sync_logout(&mut self, cx: &mut Context<Self>) {
        self.sync.send(SyncCommand::Logout);
        let mut settings = store::load_sync();
        settings.token.clear();
        let _ = store::save_sync(&mut settings);
        self.sync_notice = Some("已登出（本地数据保留）".to_string());
        cx.notify();
    }

    /// 首登对话框遮罩（普通功能实现，样式留待设计阶段）。
    fn render_first_login_overlay(
        &self,
        skin: &Skin,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let prompt = self.first_login_prompt.as_ref()?;
        let this = cx.entity();
        let counts: Vec<String> = if prompt.probe.counts_by_kind.is_empty() {
            vec!["（云端暂无实体）".to_string()]
        } else {
            prompt
                .probe
                .counts_by_kind
                .iter()
                .map(|(kind, count)| format!("{} × {}", kind.label_cn(), count))
                .collect()
        };
        let mut buttons = div().flex().gap_2().flex_wrap();
        for (label, kind, primary) in [
            ("合并（推荐）", FirstLoginPolicyKind::Merge, true),
            ("云端覆盖本地", FirstLoginPolicyKind::CloudWins, false),
            ("本地覆盖云端", FirstLoginPolicyKind::LocalWins, false),
        ] {
            let this = this.clone();
            let mut button = Button::new(eid("first-login", label))
                .compact()
                .label(label);
            button = if primary {
                button.primary()
            } else {
                button.secondary()
            };
            buttons = buttons.child(button.on_click(move |_e, window, cx| {
                this.update(cx, |this, cx| {
                    this.apply_first_login_policy(kind, window, cx)
                });
            }));
        }
        let dialog = div()
            .flex()
            .flex_col()
            .gap_3()
            .p_6()
            .w(px(420.))
            .rounded_lg()
            .bg(skin.panel)
            .border_1()
            .border_color(skin.border)
            .child(
                div()
                    .text_lg()
                    .font_semibold()
                    .text_color(skin.fg)
                    .child("首次登录：云端与本地都有数据"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .text_sm()
                    .text_color(skin.muted)
                    .child("云端内容：")
                    .children(counts.into_iter().map(|line| div().child(line))),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(skin.muted)
                    .child("「本地覆盖云端」会为云端独有数据推送删除（墓碑）。"),
            )
            .child(buttons);

        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(Hsla { a: 0.5, ..skin.fg })
                .child(dialog)
                .into_any_element(),
        )
    }
}

/// 首登三选一（UI 侧枚举，避免与核心类型耦合过紧）。
#[derive(Clone, Copy)]
pub enum FirstLoginPolicyKind {
    Merge,
    CloudWins,
    LocalWins,
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
    db.live_reminders()
        .filter(|r| !r.done && is_due(r.due_at.trim(), &stamp))
        .count()
}

/// 把已经过期的一次性提醒标记为完成（走 Mutation API，为一次正常本地改动）。
pub fn close_expired_once(db: &mut Database) {
    let today = model::today();
    let expired: Vec<String> = db
        .live_reminders()
        .filter(|r| r.repeat == Repeat::Once && !r.done)
        .filter(|r| {
            chrono::NaiveDate::parse_from_str(r.due_at.get(..10).unwrap_or(""), "%Y-%m-%d")
                .is_ok_and(|d| d < today)
        })
        .map(|r| r.id.clone())
        .collect();
    for id in expired {
        db.set_reminder_done(&id, true);
    }
}

impl Render for App {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.start_timer(cx);
        self.start_sync_consumer(cx);

        if let Some(warning) = self.startup_warning.take() {
            window.push_notification(Notification::warning(warning).title("数据文件异常"), cx);
        }

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
        let overlay = self.render_first_login_overlay(&skin, cx);

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
            .when_some(overlay, |d, overlay| d.child(overlay))
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
                    div()
                        .text_xs()
                        .text_color(muted)
                        .child(sync_status_line(&app.sync_status)),
                )
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

/// 顶栏一行的同步状态文案（含上次同步时间）。
fn sync_status_line(status: &Status) -> String {
    match status {
        Status::Idle {
            last_sync_ms: Some(ms),
        } => chrono::DateTime::from_timestamp_millis(*ms)
            .map(|dt| dt.with_timezone(&chrono::Local).format("%H:%M").to_string())
            .map(|time| format!("已同步 · {time}"))
            .unwrap_or_else(|| "已同步".to_string()),
        other => other.label_cn().to_string(),
    }
}
