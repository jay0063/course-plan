use crate::prelude::*;
use gpui::assets::IconName;
use gpui::component::Size;
use gpui::component::WindowExt;
use gpui::component::input::InputState;
use gpui::component::notification::Notification;
use gpui::component::scroll::ScrollableElement;
use gpui::*;

use crate::model::{self, Database, Repeat};
use crate::store;
use crate::sync::{
    FirstLoginPolicy, ProbeSummary, Status, SyncCommand, SyncCoreConfig, SyncEvent, SyncHandle,
};
use crate::ui::components::nav_item::nav_item as nav_item_comp;
use crate::ui::components::overlay as overlay_comp;
use crate::ui::components::overlay::FirstLoginAction;
use crate::ui::components::page_header::page_subtitle;
use crate::ui::components::sync_pill as sync_pill_comp;
use crate::ui::motion;
use crate::ui::tokens::{SIDEBAR_BRAND_H, SIDEBAR_FOOTER_H, SIDEBAR_W, TOPBAR_H};
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
    /// 首登「稍后决定」提示是否仍待展示。
    ///
    /// Cancel 后核心会立刻发一轮 `StatusChanged(Idle)`，而 `Idle` 分支默认清空
    /// `sync_notice`，会把暂缓提示抹掉。该标志令提示存活至用户下一次操作
    /// （手动「立即同步」→ `Syncing` / 登录 / 登出）。
    pub sync_defer_hint: bool,
    /// 首登对话框（双方皆非空时展示）。
    pub first_login_prompt: Option<FirstLoginPrompt>,
    /// 最近一次登录是否发生了账号切换（同账号重登时不弹首登三选一，§6）。
    pub pending_login_switched: bool,
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
            sync_defer_hint: false,
            first_login_prompt: None,
            pending_login_switched: false,
            _timer: None,
            _sync_consumer: None,
        }
    }

    /// 写入数据并刷新界面。
    ///
    /// 仅在 data.json 落盘成功后，才向同步引擎公告本地改动：写盘失败的编辑**不得**
    /// 进入同步候选（否则会推送一份本地并未持久化的状态）。
    pub fn commit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match store::save(&self.db) {
            Ok(()) => self.note_local_change(),
            Err(err) => {
                window.push_notification(Notification::error(err).title("保存失败"), cx);
            }
        }
        cx.notify();
    }

    /// 本地持久化后的钩子：投递 3 秒防抖的同步触发（§5.2）。
    pub fn note_local_change(&mut self) {
        self.sync.send(SyncCommand::LocalChanged);
    }

    /// 只持久化 data.json（远端 apply 路径；**不**触发 LocalChanged，避免回环）。
    fn persist_data(&mut self) -> Result<(), String> {
        store::save(&self.db)
    }

    pub fn goto(&mut self, page: Page, cx: &mut Context<Self>) {
        if self.page != page {
            // 换页即丢弃确认类弹层的挂起态（Gate 4 尾巴 / §7 已接受限制的收敛）：
            // 删除确认挂在 GPUI Global（courses/schedule/events 共用）、打卡归档确认
            // 挂在 thread-local——两者都不得跨页存活，否则离页再回时确认框会自己冒出来。
            cx.set_global(views::courses::PendingDelete(None));
            views::checkins::discard_archive_confirm();
        }
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
                self.on_sync_status(status);
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
                // 顺序：先 data.json（durable）→ 再回 Ack（§5.1）。写盘失败必须回
                // `ok:false`，核心据此不推进游标并向后退避重放。
                let ok = match self.persist_data() {
                    Ok(()) => true,
                    Err(err) => {
                        self.sync_notice = Some(err.clone());
                        let _ = window.update(cx, |_, window, cx| {
                            window.push_notification(
                                Notification::error(err).title("同步保存失败"),
                                cx,
                            );
                        });
                        false
                    }
                };
                self.sync.send(SyncCommand::AckRound { round_id, ok });
            }
            SyncEvent::Warning(message) => {
                self.sync_notice = Some(message.clone());
                let _ = window.update(cx, |_, window, cx| {
                    window.push_notification(Notification::warning(message).title("同步提示"), cx);
                });
            }
            SyncEvent::ProbeResult { round_id, probe } => {
                self.handle_probe(round_id, probe);
            }
            SyncEvent::RoundComplete {
                round_id,
                last_seq,
                watermark_base_ms,
            } => {
                // 先写 sync.json（游标 + 水位）；失败则回 `ok:false`，保留内存游标以便
                // 幂等重放，绝不先推进游标。
                let ok = match self.persist_round_cursors(last_seq, watermark_base_ms) {
                    Ok(()) => true,
                    Err(err) => {
                        self.sync_notice = Some(err.clone());
                        let _ = window.update(cx, |_, window, cx| {
                            window.push_notification(
                                Notification::error(err).title("同步保存失败"),
                                cx,
                            );
                        });
                        false
                    }
                };
                self.sync.send(SyncCommand::AckRound { round_id, ok });
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

    /// 状态迁移：维护提示文案的生命周期（不触发渲染，由 `on_sync_event` 统一 `notify`）。
    ///
    /// - `Syncing`：新一轮（含暂缓后手动「立即同步」触发的重新探针）开始，首登暂缓提示
    ///   失效并清空；因首登暂缓期间自动轮次被核心挡在 `pending`，此处只会在真正
    ///   重新探针时命中。
    /// - `Idle`：一轮成功收尾，清掉旧提示；但首登暂缓提示需保留至用户操作。
    fn on_sync_status(&mut self, status: Status) {
        match &status {
            Status::Syncing if self.sync_defer_hint => {
                self.sync_defer_hint = false;
                self.sync_notice = None;
            }
            Status::Idle { .. } if !self.sync_defer_hint => {
                self.sync_notice = None;
            }
            _ => {}
        }
        self.sync_status = status;
    }

    /// 探针结果：服务端空 → 全量推；本地空 → 全量拉；双方非空 → 弹三选一对话框。
    ///
    /// 同账号重登（`pending_login_switched == false`）不弹对话框：游标已被重置为 0，
    /// 全量拉取 + 推送是幂等的，直接合并即可（§6）。
    fn handle_probe(&mut self, round_id: u64, probe: ProbeSummary) {
        let server_empty = probe.all_ids.is_empty();
        // 仅 profile（账号本身的空档案，`sync_ts == 0`）不算「有数据」。
        let local_empty = self.db.all_entity_ids().len() <= 1 && self.db.profile.sync_ts == 0;
        let policy = if server_empty || !self.pending_login_switched || local_empty {
            Some(if server_empty || !self.pending_login_switched {
                FirstLoginPolicy::Merge
            } else {
                FirstLoginPolicy::CloudWins
            })
        } else {
            None
        };
        match policy {
            Some(policy) => {
                self.sync
                    .send(SyncCommand::FirstLoginPolicy { round_id, policy });
            }
            None => {
                self.first_login_prompt = Some(FirstLoginPrompt { round_id, probe });
            }
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
        // 任何登录尝试（成功或失败）都取代旧的首登暂缓提示。
        self.sync_defer_hint = false;
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
        // 记录给探针阶段：同账号重登不弹首登三选一（§6）。
        self.pending_login_switched = switched;
        let had_previous = settings.user_id.is_some() || !settings.server_url.is_empty();
        if switched && had_previous {
            // 账号切换（§3）：归档当前 data.json，本地 DB 清空为新账号空白空间。
            // 归档失败 → 完全中止切换（保留当前 db / 凭据，用户可重试）。
            if let Err(err) = self.archive_current_data(settings.user_id) {
                self.pending_login_switched = false;
                self.sync_notice = Some(format!("账号切换已中止：{err}"));
                let _ = window.update(cx, |_, window, cx| {
                    window.push_notification(
                        Notification::error(format!("账号切换已中止（本地数据未改动）：{err}"))
                            .title("同步"),
                        cx,
                    );
                });
                // 核心已切到新账号 token，登出以避免新账号数据写入旧库。
                self.sync.send(SyncCommand::Logout);
                return;
            }
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
    ///
    /// 返回 `Err` 时调用方必须**中止**账号切换（本地数据保持不变）。目标名不覆盖已存在
    /// 的归档：冲突时追加时间戳 + 序号。
    fn archive_current_data(&self, old_user_id: Option<i64>) -> Result<(), String> {
        let path = store::data_file();
        if !path.exists() {
            return Ok(());
        }
        let tag = old_user_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let mut dest = path.with_file_name(format!("data-{tag}.json"));
        if dest.exists() {
            let stamp = model::now_ms();
            dest = path.with_file_name(format!("data-{tag}-{stamp}.json"));
            let mut counter = 1u32;
            while dest.exists() {
                dest = path.with_file_name(format!("data-{tag}-{stamp}-{counter}.json"));
                counter += 1;
            }
        }
        std::fs::rename(&path, &dest)
            .map_err(|err| format!("归档 {} → {} 失败：{err}", path.display(), dest.display()))
    }

    /// 整轮完成：写 data.json（批内已写）→ 写 sync.json（游标 + 水位）→ 由调用方 ack。
    ///
    /// 返回 `Err` 时调用方应回执 `AckRound { ok: false }`，核心据此**不推进**游标/水位。
    fn persist_round_cursors(
        &mut self,
        last_seq: u64,
        watermark_base_ms: i64,
    ) -> Result<(), String> {
        let min_mut = self.db.round_min_mutation_ts();
        let watermark = match min_mut {
            Some(min) => watermark_base_ms.min(min),
            None => watermark_base_ms,
        };
        let mut settings = store::load_sync();
        settings.last_seq = last_seq;
        settings.push_watermark_ms = watermark;
        // 水位 ≤ 本轮任一本地改动戳，故内存中的 stamp_floor 抬升不会漏掉本轮改动。
        self.db.stamp_floor_ms = watermark;
        store::save_sync(&mut settings)
    }

    /// 首登对话框动作执行（§6-3）。
    ///
    /// `Cancel` 发送 [`SyncCommand::DeferFirstLogin`] 暂缓首登（不选定策略）；其余三种
    /// 转为核心 [`FirstLoginPolicy`]。
    pub fn apply_first_login_policy(
        &mut self,
        action: FirstLoginAction,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(prompt) = self.first_login_prompt.take() else {
            return;
        };
        let round_id = prompt.round_id;
        // 选定任意策略即视为已做出决定，撤销可能残留的暂缓提示。
        self.sync_defer_hint = false;
        let policy = match action {
            FirstLoginAction::Cancel => {
                // 结束探针轮并暂缓首登自动同步；「立即同步」会重新探针并再次弹窗。
                self.sync.send(SyncCommand::DeferFirstLogin { round_id });
                self.sync_notice = Some("首登选择已暂缓，点击「立即同步」重新选择。".to_string());
                // 紧接的 `StatusChanged(Idle)` 不得清空上述提示。
                self.sync_defer_hint = true;
                cx.notify();
                return;
            }
            FirstLoginAction::Merge => FirstLoginPolicy::Merge,
            FirstLoginAction::CloudWins => FirstLoginPolicy::CloudWins,
            FirstLoginAction::LocalWins => {
                // 本地覆盖云端：先重戳（压过服务端已知戳），stamp floor 一并抬高（§6-3c）。
                let base = model::now_ms().max(prompt.probe.server_time_ms.saturating_add(1));
                self.db.bump_all_stamps(base);
                self.db.stamp_floor_ms = base;
                let local_ids = self.db.all_entity_ids();
                // 把同一基址一并交给核心，使其为服务端独有 id 铸造的墓碑也压过服务端（§6-3c）。
                FirstLoginPolicy::LocalWins {
                    local_ids,
                    base_ms: base,
                }
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
        self.sync_defer_hint = false;
        self.sync_notice = Some("已登出（本地数据保留）".to_string());
        cx.notify();
    }

    /// 首登对话框遮罩（§3.13：走 `ui::components::overlay` + `first_login_prompt`）。
    fn render_first_login_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let prompt = self.first_login_prompt.as_ref()?;
        let theme = cx.theme().clone();
        let this = cx.entity();
        let dialog =
            overlay_comp::first_login_prompt(&theme, &prompt.probe, move |action, window, cx| {
                this.update(cx, |this, cx| {
                    this.apply_first_login_policy(action, window, cx)
                });
            });
        Some(overlay_comp::overlay(&theme, dialog).into_any_element())
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
        let overlay = self.render_first_login_overlay(cx);

        // §3.18 页面切换：opacity 0→1 + translateY(4px→0)，260ms，每页唯一 id。
        // `with_animation` 内部自动遵守 reduce-motion。
        // 注意：wrapper 必须 `size_full()`（父层有确定高度），否则子内容的
        // 百分比高度会相对 auto 解析而塌缩成 0。
        let page_enter = div().size_full().child(content).with_animation(
            ElementId::from(format!("page-enter:{}", page.label())),
            motion::page_enter_animation(),
            |el, delta| el.opacity(delta).mt(px(4. * (1.0 - delta))),
        );

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
                    .child(div().flex_1().min_h_0().overflow_hidden().child(page_enter)),
            )
            .when_some(overlay, |d, overlay| d.child(overlay))
    }
}

fn render_sidebar(app: &App, _skin: &Skin, cx: &mut Context<App>) -> AnyElement {
    let theme = cx.theme();
    let this = cx.entity();
    let page = app.page;
    let name = if app.db.profile.student.is_empty() {
        "课程计划".to_string()
    } else {
        format!("{}的课程", app.db.profile.student)
    };
    let grade = app.db.profile.grade.clone();
    let today = model::today();

    // 品牌块：32×32 圆角方糖（"课" 字 + primary 色）+ 姓名 + 年级。
    let brand = div()
        .id("ui:brand:")
        .flex()
        .items_center()
        .gap(px(12.))
        .px(px(20.))
        .h(SIDEBAR_BRAND_H)
        .child(
            div()
                .size(px(32.))
                .rounded(px(10.))
                .flex()
                .items_center()
                .justify_center()
                .bg(theme.primary)
                .child(
                    div()
                        .text_base()
                        .font_weight(FontWeight(700.0))
                        .text_color(theme.primary_foreground)
                        .child("课"),
                ),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight(600.0))
                        .text_color(theme.foreground)
                        .child(name),
                )
                .when(!grade.is_empty(), |d| {
                    d.child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(grade.clone()),
                    )
                }),
        )
        .test_support();

    // nav 列表（使用 `nav_item` 组件；测试 id 与 Phase 2 保持一致）。
    let mut items = div().flex().flex_col().gap(px(0.)).p(px(8.));
    for p in Page::ALL {
        let active = p == page;
        let this = this.clone();
        // badge 颜色按语义（§3.3）：过期提醒 danger、待打卡 warning。
        let badge: Option<(usize, Hsla)> = match p {
            Page::Reminders => {
                let n = pending_reminders(&app.db);
                (n > 0).then_some((n, theme.danger))
            }
            Page::CheckIns => {
                let n = app
                    .db
                    .checkins_on(today)
                    .into_iter()
                    .filter(|c| !c.done_on(today))
                    .count();
                (n > 0).then_some((n, theme.warning))
            }
            _ => None,
        };
        items = items.child(nav_item_comp(
            theme,
            eid("nav", p.label()),
            p.icon(),
            p.label(),
            active,
            badge,
            move |_e, _w, cx| {
                this.update(cx, |this, cx| this.goto(p, cx));
            },
        ));
    }

    // 底部：日期 + compact 同步状态（点击跳设置页）。
    let this_sync = this.clone();
    let footer = div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(8.))
        .px(px(20.))
        .h(SIDEBAR_FOOTER_H)
        .border_t_1()
        .border_color(theme.border)
        .child(
            div()
                .text_xs()
                .text_color(theme.muted_foreground)
                .child(model::fmt_date_cn(today)),
        )
        .child(sync_pill_comp::sync_indicator_compact(
            theme,
            eid("ui:sidebar", "sync-indicator"),
            &app.sync_status,
            move |_e, _w, cx| {
                this_sync.update(cx, |this, cx| this.goto(Page::Settings, cx));
            },
        ));

    div()
        .flex()
        .flex_col()
        .h_full()
        .w(SIDEBAR_W)
        .flex_shrink_0()
        .bg(theme.sidebar)
        .border_r_1()
        .border_color(theme.border)
        .child(brand)
        .child(
            div()
                .id("ui:sidebar:nav")
                .flex_1()
                .overflow_y_scrollbar()
                .child(items),
        )
        .child(footer)
        .into_any_element()
}

fn render_topbar(app: &App, _skin: &Skin, cx: &mut Context<App>) -> AnyElement {
    let theme = cx.theme();
    let this = cx.entity();
    let page = app.page;
    let dark = theme.is_dark();
    let subtitle = page_subtitle(page, &app.db);

    // 左：page icon + title + subtitle
    let title_block = div()
        .flex()
        .flex_col()
        .gap(px(2.))
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(
                    Icon::new(page.icon())
                        .with_size(Size::Small)
                        .text_color(theme.primary),
                )
                .child(
                    div()
                        .text_size(px(20.))
                        .font_weight(FontWeight(700.0))
                        .text_color(theme.foreground)
                        .child(page.label()),
                ),
        )
        .child(
            div()
                .text_sm()
                .text_color(theme.muted_foreground)
                .child(subtitle),
        );

    // 右：sync_pill + theme toggle (icon_only) + 快速添加 (primary)
    let this_for_pill = this.clone();
    let sync_pill_any = sync_pill_comp::sync_pill(
        theme,
        ElementId::from("ui:topbar:sync-pill"),
        &app.sync_status,
        move |_e, _w, cx| {
            this_for_pill.update(cx, |this, cx| {
                this.goto(Page::Settings, cx);
            });
        },
    )
    .into_any_element();

    let this_for_quick_add = this.clone();
    div()
        .flex()
        .items_center()
        .justify_between()
        .gap(px(12.))
        .h(TOPBAR_H)
        .px(px(24.))
        .flex_shrink_0()
        .bg(theme.popover)
        .border_b_1()
        .border_color(theme.border)
        .child(title_block)
        .child(
            div()
                .flex()
                .items_center()
                .gap(px(8.))
                .child(sync_pill_any)
                .child(crate::ui::components::button::icon_only(
                    "ui:topbar:theme-toggle",
                    if dark { IconName::Sun } else { IconName::Moon },
                    if dark {
                        "切换到浅色主题"
                    } else {
                        "切换到深色主题"
                    },
                    move |_e, window, app| {
                        crate::theme::toggle_mode(Some(window), app);
                    },
                ))
                .child(crate::ui::components::button::primary_with_icon(
                    "ui:topbar:quick-add",
                    "快速添加",
                    IconName::Plus,
                    move |_e, window, cx| {
                        this_for_quick_add.update(cx, |this, cx| this.quick_add(window, cx));
                    },
                )),
        )
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headless_app() -> App {
        App::with_database(Database::default())
    }

    /// Cancel → `StatusChanged(Idle)` → 提示仍可见 → `SyncNow`（`Syncing`）→ 提示清除。
    #[::core::prelude::v1::test]
    fn defer_hint_survives_idle_then_clears_on_syncing() {
        let mut app = headless_app();
        // 模拟「稍后决定」：写入暂缓提示并置标志。
        app.sync_notice = Some("首登选择已暂缓，点击「立即同步」重新选择。".to_string());
        app.sync_defer_hint = true;

        // 核心紧随 Cancel 发来的一轮 Idle 不得抹掉提示。
        app.on_sync_status(Status::Idle { last_sync_ms: None });
        assert!(app.sync_defer_hint);
        assert_eq!(
            app.sync_notice.as_deref(),
            Some("首登选择已暂缓，点击「立即同步」重新选择。")
        );

        // 手动「立即同步」→ 核心重新探针 → `Syncing`：提示清除。
        app.on_sync_status(Status::Syncing);
        assert!(!app.sync_defer_hint);
        assert!(app.sync_notice.is_none());
    }

    /// 普通提示（如登录结果）仍按原逻辑在回到 Idle 时清空。
    #[::core::prelude::v1::test]
    fn idle_still_clears_normal_notice() {
        let mut app = headless_app();
        app.sync_notice = Some("已登录：alice".to_string());
        app.on_sync_status(Status::Idle { last_sync_ms: None });
        assert!(app.sync_notice.is_none());
    }
}
