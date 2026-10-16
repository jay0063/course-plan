//! 同步引擎核心：**无 UI、无线程、无文件** 的状态机（协议 §5 / §6）。
//!
//! 设计要点（对齐 `docs/SYNC-PROTOCOL.md` v1.2，接口见 `.slim/.../progress.md`）：
//!
//! - 核心只负责 **HTTP 请求 + 轮次时序**；数据库与 `sync.json` 一律由 UI 线程读写
//!   （单写者契约，§5.1）。核心从不读 `Database`、从不写文件。
//! - 候选集合由 UI 收集：核心发 [`SyncEvent::NeedCandidates`]，UI 回
//!   [`SyncCommand::Candidates`]（用 `all_changes(watermark)`）。
//! - 每批远端变更 → [`SyncEvent::ApplyBatch`] → UI `apply_remote` + 持久化 data.json
//!   → [`SyncCommand::AckRound`] → 核心继续。**游标只有整轮结束后（UI 写完
//!   data.json 再写 sync.json）才推进**（§5.1 崩溃语义）。
//! - 时间全部经注入的 [`Clock`]（`fn() -> i64`），便于确定性测试。
//! - 网络抽象为 [`SyncApi`]：生产实现是 reqwest（worker），测试可用假实现或
//!   直接对着进程内真实服务端。

use std::collections::VecDeque;

use course_plan_sync_proto::{
    AuthResponse, Change, Credentials, EntityKind, HealthResponse, MAX_PAYLOAD_BYTES,
    MAX_PUSH_BYTES, MAX_PUSH_CHANGES, PullResponse, PushResponse, outbound_cap,
};

/// 可注入时钟：返回当前 unix 毫秒。
pub type Clock = Box<dyn Fn() -> i64 + Send + Sync>;

/// `LocalChanged` 防抖窗口（协议 §5.2）。
pub const DEBOUNCE_MS: i64 = 3_000;
/// 周期同步间隔（协议 §5.2）。
pub const PERIODIC_MS: i64 = 5 * 60_000;
/// 失败退避序列（协议 §5.3）：5s → 15s → 60s → 300s（封顶）。
pub const RETRY_DELAYS_MS: [i64; 4] = [5_000, 15_000, 60_000, 300_000];

// ─────────────────────────────── 网络抽象 ───────────────────────────────

/// 网络层错误分类（核心据此决定状态与退避）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiError {
    /// 401 / 403：凭据失效 → `AuthFailed`，停止自动轮询。
    Unauthorized,
    /// 连接失败 / 超时等 → `Offline` + 自动重试。
    Network(String),
    /// 其它非 2xx：带 HTTP 状态码与服务端错误码（若有）。
    Status {
        status: u16,
        code: Option<String>,
        message: String,
    },
}

/// 同步 HTTP 客户端抽象。实现见 `worker::ReqwestApi`。
///
/// 使用 `async fn` in trait（AFIT）：核心泛型于 `A: SyncApi`，无需 `dyn`。
#[allow(async_fn_in_trait)] // rustc lint：公开 trait 的 auto trait bound 由泛型参数承载
pub trait SyncApi {
    async fn login(
        &self,
        server_url: &str,
        credentials: &Credentials,
    ) -> Result<AuthResponse, ApiError>;
    async fn register(
        &self,
        server_url: &str,
        credentials: &Credentials,
    ) -> Result<AuthResponse, ApiError>;
    async fn push(
        &self,
        server_url: &str,
        token: &str,
        changes: &[Change],
    ) -> Result<PushResponse, ApiError>;
    async fn pull(
        &self,
        server_url: &str,
        token: &str,
        since: u64,
    ) -> Result<PullResponse, ApiError>;
    async fn health(&self, server_url: &str) -> Result<HealthResponse, ApiError>;
}

// ─────────────────────────────── 命令 / 事件 ───────────────────────────────

/// 同步状态（worker → UI）。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    LoggedOut,
    Idle { last_sync_ms: Option<i64> },
    Syncing,
    Offline,
    Error { msg: String },
    AuthFailed,
}

impl Status {
    /// 顶栏一行的中文状态文案。
    pub fn label_cn(&self) -> &'static str {
        match self {
            Status::LoggedOut => "未登录",
            Status::Idle { .. } => "已同步",
            Status::Syncing => "同步中…",
            Status::Offline => "离线",
            Status::Error { .. } => "同步错误",
            Status::AuthFailed => "登录失效",
        }
    }
}

/// 首登三选一策略（协议 §6）。
#[derive(Clone, Debug)]
pub enum FirstLoginPolicy {
    /// 合并（推荐）：全量 pull + 全量 push（LWW）。
    Merge,
    /// 云端覆盖本地：丢弃本地未同步数据（不产生墓碑）。
    CloudWins,
    /// 本地覆盖云端：重戳本地（UI 已做）+ 为服务端独有 id 推墓碑 + 全量 push。
    LocalWins {
        local_ids: Vec<(EntityKind, String)>,
    },
}

/// 探针结果（协议 §6：`since=0` 只统计不应用）。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProbeSummary {
    /// 存活实体按类型计数（墓碑不计入）。
    pub counts_by_kind: Vec<(EntityKind, usize)>,
    pub latest_seq: u64,
    pub server_time_ms: i64,
    /// 服务端**存活**实体 id 全集合（用于 LocalWins 差集）。
    pub all_ids: Vec<(EntityKind, String)>,
}

/// UI → 核心 命令。
#[derive(Clone, Debug)]
pub enum SyncCommand {
    Login {
        server_url: String,
        username: String,
        password: String,
        register: bool,
    },
    Logout,
    /// 手动「立即同步」：绕过防抖，立即开轮。
    SyncNow,
    /// 本地 commit 后触发：3 秒防抖，合并多次。
    LocalChanged,
    /// UI 回填候选集合（响应 [`SyncEvent::NeedCandidates`]）。
    Candidates {
        round_id: u64,
        changes: Vec<Change>,
    },
    /// UI 持久化回执，核心据此推进轮次。`ok=false` 表示持久化失败：核心**不推进
    /// 游标/水位**，走常规失败路径（退避重试），由幂等重放修正。
    AckRound {
        round_id: u64,
        ok: bool,
    },
    /// 首登策略选定（响应 [`SyncEvent::ProbeResult`]）。
    FirstLoginPolicy {
        round_id: u64,
        policy: FirstLoginPolicy,
    },
}

/// 核心 → UI 事件。
#[derive(Clone, Debug)]
pub enum SyncEvent {
    StatusChanged(Status),
    /// 轮次开始（UI 应在此调用 `Database::begin_round`）。
    RoundStarted {
        round_id: u64,
    },
    /// 请求候选集合。`full=true` → 忽略水位取全部（首登策略轮）。
    NeedCandidates {
        round_id: u64,
        full: bool,
    },
    /// 一批远端/被覆盖变更，UI 应 `apply_remote` + 持久化 data.json 后回执 AckRound。
    ApplyBatch {
        round_id: u64,
        changes: Vec<Change>,
        /// 云端覆盖本地：UI 先重置数据库再逐条 apply（不产生墓碑）。
        replace_all: bool,
    },
    /// 非致命提示（如单条 payload 超限被跳过），UI 应展示一次（通知 / sync_notice）。
    Warning(String),
    /// 首登探针结果，UI 据此弹对话框并回 [`SyncCommand::FirstLoginPolicy`]。
    ProbeResult {
        round_id: u64,
        probe: ProbeSummary,
    },
    /// 整轮完成：UI 应写 data.json（各批已写）后写 sync.json（游标 + 水位）再回执 AckRound。
    RoundComplete {
        round_id: u64,
        last_seq: u64,
        /// `min(C0, 最终 server_time_ms)`；UI 再取 `min(., round_min_mut_ts)` 作为最终水位。
        watermark_base_ms: i64,
    },
    /// 登录 / 注册结果。
    AuthResult {
        ok: bool,
        message: String,
        server_url: Option<String>,
        user_id: Option<i64>,
        username: Option<String>,
        /// 成功时的会话 token（UI 需持久化进 sync.json）。
        token: Option<String>,
    },
}

// ─────────────────────────────── 核心构造 ───────────────────────────────

/// 核心初始配置（来自 `sync.json`，由 UI 读取）。
#[derive(Clone, Debug, Default)]
pub struct SyncCoreConfig {
    pub device_id: String,
    pub server_url: String,
    pub username: String,
    pub user_id: Option<i64>,
    pub token: String,
    /// 已持久化的 pull 游标。
    pub last_seq: u64,
}

#[derive(Clone, Debug)]
enum Mode {
    Normal,
    Merge,
    CloudWins,
    LocalWins {
        server_only: Vec<(EntityKind, String)>,
    },
}

#[derive(Clone, Copy, Debug)]
enum Next {
    Pull {
        since: u64,
        first: bool,
        resume: ResumeAfter,
    },
    Finalize {
        latest_seq: u64,
    },
    /// 预拉取（首登 Merge）完成后转入候选 → 推送阶段。
    Candidates {
        latest_seq: u64,
    },
}

/// pull 分页全部读完后（`has_more=false`）的归属。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ResumeAfter {
    /// 收尾轮次（推进游标）。
    Finalize,
    /// 首登 Merge 预拉取：应用后再推本地（`latest_seq` 作为 push 后增量拉取的 since）。
    Candidates,
}

#[derive(Clone, Debug)]
enum Phase {
    Idle,
    Probe {
        round_id: u64,
        since: u64,
        probe: ProbeSummary,
    },
    AwaitPolicy {
        round_id: u64,
        probe: ProbeSummary,
    },
    AwaitCandidates {
        round_id: u64,
        c0: i64,
        mode: Mode,
        /// 首登 Merge 预拉取完成时的 `latest_seq`；作为 push 后增量拉取的 since。
        pull_from: Option<u64>,
    },
    Push {
        round_id: u64,
        c0: i64,
        mode: Mode,
        batches: VecDeque<Vec<Change>>,
        overridden: Vec<Change>,
        pull_from: u64,
    },
    Pull {
        round_id: u64,
        c0: i64,
        mode: Mode,
        since: u64,
        first: bool,
        resume: ResumeAfter,
    },
    AwaitAck {
        round_id: u64,
        c0: i64,
        mode: Mode,
        next: Next,
    },
    Finalize {
        round_id: u64,
        c0: i64,
        latest_seq: u64,
    },
    AwaitFinalAck {
        round_id: u64,
        latest_seq: u64,
    },
}

/// 同步引擎核心（泛型于网络实现 `A`）。
pub struct SyncCore<A: SyncApi> {
    api: A,
    clock: Clock,
    device_id: String,
    server_url: String,
    username: String,
    user_id: Option<i64>,
    token: String,
    last_seq: u64,
    last_server_time_ms: Option<i64>,
    status: Status,
    phase: Phase,
    round_counter: u64,
    pending: bool,
    debounce_at: Option<i64>,
    next_periodic_at: Option<i64>,
    retry_at: Option<i64>,
    retry_index: usize,
    last_sync_ms: Option<i64>,
}

impl<A: SyncApi> SyncCore<A> {
    pub fn new(api: A, clock: Clock, config: SyncCoreConfig) -> Self {
        let status = if config.token.is_empty() {
            Status::LoggedOut
        } else {
            Status::Idle { last_sync_ms: None }
        };
        Self {
            api,
            clock,
            device_id: config.device_id,
            server_url: config.server_url,
            username: config.username,
            user_id: config.user_id,
            token: config.token,
            last_seq: config.last_seq,
            last_server_time_ms: None,
            status,
            phase: Phase::Idle,
            round_counter: 0,
            pending: false,
            debounce_at: None,
            next_periodic_at: None,
            retry_at: None,
            retry_index: 0,
            last_sync_ms: None,
        }
    }

    pub fn status(&self) -> Status {
        self.status.clone()
    }

    pub fn last_seq(&self) -> u64 {
        self.last_seq
    }

    /// 处理一条命令，返回需要发给 UI 的事件。
    pub async fn handle(&mut self, command: SyncCommand) -> Vec<SyncEvent> {
        let mut events = Vec::new();
        match command {
            SyncCommand::Login {
                server_url,
                username,
                password,
                register,
            } => {
                self.server_url = server_url;
                self.username = username.clone();
                let credentials = Credentials { username, password };
                let result = if register {
                    self.api.register(&self.server_url, &credentials).await
                } else {
                    self.api.login(&self.server_url, &credentials).await
                };
                match result {
                    Ok(auth) => {
                        self.token = auth.token.clone();
                        self.user_id = Some(auth.user.id);
                        self.username = auth.user.username.clone();
                        if self.device_id.trim().is_empty() {
                            // 防御：确保出站变更携带设备标识（正常由 UI 持久化提供）。
                            self.device_id = format!("dev-{}-{}", (self.clock)(), auth.user.id);
                        }
                        // 登录成功 → 探针（§6）。游标在策略选定后按策略重置。
                        // 重新登录清除旧的退避（避免沿用上一次会话的长退避）。
                        self.retry_at = None;
                        self.retry_index = 0;
                        self.last_server_time_ms = None;
                        events.push(SyncEvent::AuthResult {
                            ok: true,
                            message: "登录成功".to_string(),
                            server_url: Some(self.server_url.clone()),
                            user_id: Some(auth.user.id),
                            username: Some(auth.user.username),
                            token: Some(auth.token),
                        });
                        let round_id = self.next_round_id();
                        self.status = Status::Syncing;
                        events.push(SyncEvent::StatusChanged(Status::Syncing));
                        self.phase = Phase::Probe {
                            round_id,
                            since: 0,
                            probe: ProbeSummary::default(),
                        };
                    }
                    Err(err) => {
                        events.push(SyncEvent::AuthResult {
                            ok: false,
                            message: auth_failure_message(&err),
                            server_url: Some(self.server_url.clone()),
                            user_id: None,
                            username: None,
                            token: None,
                        });
                    }
                }
            }
            SyncCommand::Logout => {
                self.token.clear();
                self.user_id = None;
                self.pending = false;
                self.debounce_at = None;
                self.retry_at = None;
                self.next_periodic_at = None;
                self.phase = Phase::Idle;
                self.set_status(Status::LoggedOut, &mut events);
            }
            SyncCommand::SyncNow => {
                self.trigger(true, &mut events).await;
            }
            SyncCommand::LocalChanged => {
                self.trigger(false, &mut events).await;
            }
            SyncCommand::Candidates { round_id, changes } => {
                self.on_candidates(round_id, changes, &mut events);
            }
            SyncCommand::AckRound { round_id, ok } => {
                self.on_ack(round_id, ok, &mut events);
            }
            SyncCommand::FirstLoginPolicy { round_id, policy } => {
                self.on_policy(round_id, policy, &mut events);
            }
        }
        self.drive(&mut events).await;
        events
    }

    /// 周期性驱动（worker 每秒调用；测试可直接调用）：
    /// 处理退避重试、防抖到点、5 分钟周期同步。
    pub async fn tick(&mut self) -> Vec<SyncEvent> {
        let now = (self.clock)();
        let mut events = Vec::new();

        if matches!(self.status, Status::Offline | Status::Error { .. })
            && let Some(at) = self.retry_at
            && now >= at
        {
            self.retry_at = None;
            self.start_round(Mode::Normal, &mut events);
        }

        if matches!(self.phase, Phase::Idle)
            && !matches!(
                self.status,
                Status::Offline | Status::Error { .. } | Status::AuthFailed | Status::LoggedOut
            )
        {
            if let Some(at) = self.debounce_at
                && now >= at
            {
                self.debounce_at = None;
                self.start_round(Mode::Normal, &mut events);
            }
            if !self.token.is_empty() && !matches!(self.status, Status::Syncing) {
                match self.next_periodic_at {
                    None => self.next_periodic_at = Some(now.saturating_add(PERIODIC_MS)),
                    Some(at) if now >= at => {
                        self.next_periodic_at = Some(now.saturating_add(PERIODIC_MS));
                        self.start_round(Mode::Normal, &mut events);
                    }
                    Some(_) => {}
                }
            }
        }

        self.drive(&mut events).await;
        events
    }

    // MARK: - 触发

    fn next_round_id(&mut self) -> u64 {
        self.round_counter = self.round_counter.wrapping_add(1);
        self.round_counter
    }

    async fn trigger(&mut self, immediate: bool, events: &mut Vec<SyncEvent>) {
        if self.token.is_empty() || matches!(self.status, Status::AuthFailed) {
            return;
        }
        if !matches!(self.phase, Phase::Idle) {
            // 单飞：轮内触发只置 pending（§5.2）。
            self.pending = true;
            return;
        }
        if immediate {
            self.debounce_at = None;
            self.start_round(Mode::Normal, events);
        } else {
            self.debounce_at = Some((self.clock)().saturating_add(DEBOUNCE_MS));
        }
    }

    fn start_round(&mut self, mode: Mode, events: &mut Vec<SyncEvent>) {
        if !matches!(self.phase, Phase::Idle) {
            self.pending = true;
            return;
        }
        let round_id = self.next_round_id();
        let c0 = (self.clock)();
        self.set_status(Status::Syncing, events);
        events.push(SyncEvent::RoundStarted { round_id });
        match mode {
            Mode::CloudWins => {
                // 云端覆盖本地：无需候选，直接全量 pull（首批 replace_all）。
                self.phase = Phase::Pull {
                    round_id,
                    c0,
                    mode,
                    since: 0,
                    first: true,
                    resume: ResumeAfter::Finalize,
                };
            }
            Mode::Merge => {
                // 首登合并（§6-3a）：先全量拉取并应用，再推本地，最后拉增量收尾。
                self.phase = Phase::Pull {
                    round_id,
                    c0,
                    mode,
                    since: 0,
                    first: true,
                    resume: ResumeAfter::Candidates,
                };
            }
            other => {
                let full = !matches!(other, Mode::Normal);
                self.phase = Phase::AwaitCandidates {
                    round_id,
                    c0,
                    mode: other,
                    pull_from: None,
                };
                events.push(SyncEvent::NeedCandidates { round_id, full });
            }
        }
    }

    // MARK: - 命令处理

    fn on_candidates(&mut self, round_id: u64, changes: Vec<Change>, events: &mut Vec<SyncEvent>) {
        let phase = std::mem::replace(&mut self.phase, Phase::Idle);
        let (c0, mode, resume_pull_from) = match phase {
            Phase::AwaitCandidates {
                round_id: rid,
                c0,
                mode,
                pull_from,
            } if rid == round_id => (c0, mode, pull_from),
            other => {
                // 非预期/过期回执：恢复原状态。
                self.phase = other;
                return;
            }
        };
        let mut all = changes;
        if let Mode::LocalWins { server_only } = &mode {
            let now = (self.clock)();
            for (kind, id) in server_only {
                all.push(Change {
                    kind: *kind,
                    id: id.clone(),
                    updated_at: now,
                    deleted: true,
                    device_id: String::new(),
                    payload: None,
                    server_seq: None,
                });
            }
        }
        // 客户端侧超限保护：单条 payload 超上限的变更跳过（否则服务端 413 会陷入
        // 无限退避），并一次性告警（§4）。
        all = drop_oversize(all, events);
        // 协议 §5.3：按 ts 升序推送。
        all.sort_by_key(|c| c.updated_at);
        let batches = shard(all, &self.device_id, self.last_server_time_ms);
        let pull_from = match mode {
            Mode::Normal => self.last_seq,
            // 首登 Merge：预拉取已读到的 latest_seq，作为 push 后增量拉取的 since。
            Mode::Merge => resume_pull_from.unwrap_or(0),
            _ => 0,
        };
        self.phase = Phase::Push {
            round_id,
            c0,
            mode,
            batches: batches.into(),
            overridden: Vec::new(),
            pull_from,
        };
    }

    fn on_ack(&mut self, round_id: u64, ok: bool, events: &mut Vec<SyncEvent>) {
        let phase = std::mem::replace(&mut self.phase, Phase::Idle);
        if !ok {
            // 本地持久化失败：**不推进**游标/水位，走常规失败路径（退避重试）。
            // 内存状态保留，重试轮自旧游标重放，apply 幂等。
            let expected = match &phase {
                Phase::AwaitAck { round_id: rid, .. } => *rid == round_id,
                Phase::AwaitFinalAck { round_id: rid, .. } => *rid == round_id,
                _ => false,
            };
            if expected {
                self.fail(
                    ApiError::Status {
                        status: 0,
                        code: None,
                        message: "本地持久化失败，稍后重试".to_string(),
                    },
                    events,
                );
            } else {
                // 过期 / 非预期回执：忽略。
                self.phase = phase;
            }
            return;
        }
        match phase {
            Phase::AwaitAck {
                round_id: rid,
                c0,
                mode,
                next,
            } if rid == round_id => {
                self.advance(round_id, c0, mode, next, events);
            }
            Phase::AwaitFinalAck {
                round_id: rid,
                latest_seq,
            } if rid == round_id => {
                // UI 已写 data.json → sync.json；此处仅推进内存游标（§5.1）。
                self.last_seq = latest_seq;
                self.retry_at = None;
                self.retry_index = 0;
                let now = (self.clock)();
                self.last_sync_ms = Some(now);
                self.set_status(
                    Status::Idle {
                        last_sync_ms: Some(now),
                    },
                    events,
                );
                self.phase = Phase::Idle;
                if self.pending {
                    self.pending = false;
                    self.start_round(Mode::Normal, events);
                }
            }
            other => {
                // 过期 / 非预期回执：忽略。
                self.phase = other;
            }
        }
    }

    /// 依据 [`Next`] 推进到下一阶段（可能发出 [`SyncEvent::NeedCandidates`]）。
    fn advance(
        &mut self,
        round_id: u64,
        c0: i64,
        mode: Mode,
        next: Next,
        events: &mut Vec<SyncEvent>,
    ) {
        match next {
            Next::Pull {
                since,
                first,
                resume,
            } => {
                self.phase = Phase::Pull {
                    round_id,
                    c0,
                    mode,
                    since,
                    first,
                    resume,
                };
            }
            Next::Finalize { latest_seq } => {
                self.phase = Phase::Finalize {
                    round_id,
                    c0,
                    latest_seq,
                };
            }
            Next::Candidates { latest_seq } => {
                // 首登 Merge 预拉取完成 → 应用后再推本地。
                self.phase = Phase::AwaitCandidates {
                    round_id,
                    c0,
                    mode,
                    pull_from: Some(latest_seq),
                };
                events.push(SyncEvent::NeedCandidates {
                    round_id,
                    full: true,
                });
            }
        }
    }

    fn on_policy(&mut self, round_id: u64, policy: FirstLoginPolicy, events: &mut Vec<SyncEvent>) {
        let phase = std::mem::replace(&mut self.phase, Phase::Idle);
        let probe = match phase {
            Phase::AwaitPolicy {
                round_id: rid,
                probe,
            } if rid == round_id => probe,
            other => {
                self.phase = other;
                return;
            }
        };
        // 任何首登策略都从全量游标起点开始（协议 §3：切换/首登重置游标）。
        self.last_seq = 0;
        let mode = match policy {
            FirstLoginPolicy::Merge => Mode::Merge,
            FirstLoginPolicy::CloudWins => Mode::CloudWins,
            FirstLoginPolicy::LocalWins { local_ids } => {
                let server_only = probe
                    .all_ids
                    .iter()
                    .filter(|entry| !local_ids.contains(entry))
                    .cloned()
                    .collect();
                Mode::LocalWins { server_only }
            }
        };
        self.start_round(mode, events);
    }

    // MARK: - 驱动循环

    async fn drive(&mut self, events: &mut Vec<SyncEvent>) {
        loop {
            let phase = std::mem::replace(&mut self.phase, Phase::Idle);
            match phase {
                Phase::Idle | Phase::AwaitCandidates { .. } | Phase::AwaitPolicy { .. } => {
                    self.phase = phase;
                    break;
                }
                Phase::AwaitAck { .. } | Phase::AwaitFinalAck { .. } => {
                    self.phase = phase;
                    break;
                }
                Phase::Probe {
                    round_id,
                    since,
                    mut probe,
                } => match self.api.pull(&self.server_url, &self.token, since).await {
                    Ok(resp) => {
                        self.last_server_time_ms = Some(resp.server_time_ms);
                        probe.latest_seq = resp.latest_seq;
                        probe.server_time_ms = resp.server_time_ms;
                        for change in &resp.changes {
                            if change.deleted {
                                continue;
                            }
                            probe.all_ids.push((change.kind, change.id.clone()));
                            match probe
                                .counts_by_kind
                                .iter_mut()
                                .find(|(kind, _)| *kind == change.kind)
                            {
                                Some((_, count)) => *count += 1,
                                None => probe.counts_by_kind.push((change.kind, 1)),
                            }
                        }
                        if resp.has_more {
                            let next = next_cursor(&resp, since);
                            match next {
                                Some(next) => {
                                    self.phase = Phase::Probe {
                                        round_id,
                                        since: next,
                                        probe,
                                    };
                                    continue;
                                }
                                None => {
                                    self.fail(
                                        ApiError::Status {
                                            status: 500,
                                            code: None,
                                            message: "pull 游标未前进".to_string(),
                                        },
                                        events,
                                    );
                                    break;
                                }
                            }
                        }
                        events.push(SyncEvent::ProbeResult {
                            round_id,
                            probe: probe.clone(),
                        });
                        self.phase = Phase::AwaitPolicy { round_id, probe };
                        break;
                    }
                    Err(err) => {
                        self.fail(err, events);
                        break;
                    }
                },
                Phase::Push {
                    round_id,
                    c0,
                    mode,
                    mut batches,
                    mut overridden,
                    pull_from,
                } => {
                    if let Some(batch) = batches.pop_front() {
                        match self.api.push(&self.server_url, &self.token, &batch).await {
                            Ok(resp) => {
                                self.last_server_time_ms = Some(resp.server_time_ms);
                                overridden.extend(resp.overridden);
                                self.phase = Phase::Push {
                                    round_id,
                                    c0,
                                    mode,
                                    batches,
                                    overridden,
                                    pull_from,
                                };
                                continue;
                            }
                            Err(err) => {
                                self.fail(err, events);
                                break;
                            }
                        }
                    }
                    // 推送完毕：应用 overridden（若有），否则直接进入 pull。
                    if overridden.is_empty() {
                        self.phase = Phase::Pull {
                            round_id,
                            c0,
                            mode,
                            since: pull_from,
                            first: true,
                            resume: ResumeAfter::Finalize,
                        };
                        continue;
                    }
                    events.push(SyncEvent::ApplyBatch {
                        round_id,
                        changes: overridden,
                        replace_all: false,
                    });
                    self.phase = Phase::AwaitAck {
                        round_id,
                        c0,
                        mode,
                        next: Next::Pull {
                            since: pull_from,
                            first: true,
                            resume: ResumeAfter::Finalize,
                        },
                    };
                    break;
                }
                Phase::Pull {
                    round_id,
                    c0,
                    mode,
                    since,
                    first,
                    resume,
                } => match self.api.pull(&self.server_url, &self.token, since).await {
                    Ok(resp) => {
                        self.last_server_time_ms = Some(resp.server_time_ms);
                        let replace_all = first && matches!(mode, Mode::CloudWins);
                        let changes = resp.changes.clone();
                        let has_more = resp.has_more;
                        let next = if has_more {
                            match next_cursor(&resp, since) {
                                Some(next) => Next::Pull {
                                    since: next,
                                    first: false,
                                    resume,
                                },
                                None => {
                                    self.fail(
                                        ApiError::Status {
                                            status: 500,
                                            code: None,
                                            message: "pull 游标未前进".to_string(),
                                        },
                                        events,
                                    );
                                    break;
                                }
                            }
                        } else {
                            match resume {
                                // 首登 Merge 预拉取完成 → 转推送。
                                ResumeAfter::Candidates => Next::Candidates {
                                    latest_seq: resp.latest_seq,
                                },
                                ResumeAfter::Finalize => Next::Finalize {
                                    latest_seq: resp.latest_seq,
                                },
                            }
                        };

                        if changes.is_empty() && !replace_all {
                            // 空批无需 UI 回执：直接推进。
                            self.advance(round_id, c0, mode, next, events);
                            continue;
                        }
                        events.push(SyncEvent::ApplyBatch {
                            round_id,
                            changes,
                            replace_all,
                        });
                        self.phase = Phase::AwaitAck {
                            round_id,
                            c0,
                            mode,
                            next,
                        };
                        break;
                    }
                    Err(err) => {
                        self.fail(err, events);
                        break;
                    }
                },
                Phase::Finalize {
                    round_id,
                    c0,
                    latest_seq,
                } => {
                    let server_time = self.last_server_time_ms.unwrap_or(c0);
                    let watermark_base_ms = c0.min(server_time);
                    events.push(SyncEvent::RoundComplete {
                        round_id,
                        last_seq: latest_seq,
                        watermark_base_ms,
                    });
                    self.phase = Phase::AwaitFinalAck {
                        round_id,
                        latest_seq,
                    };
                    break;
                }
            }
        }
    }

    // MARK: - 失败处理

    fn fail(&mut self, err: ApiError, events: &mut Vec<SyncEvent>) {
        self.phase = Phase::Idle;
        match err {
            ApiError::Unauthorized => {
                self.pending = false;
                self.retry_at = None;
                self.set_status(Status::AuthFailed, events);
            }
            ApiError::Network(message) => {
                self.schedule_retry();
                let _ = message;
                self.set_status(Status::Offline, events);
            }
            ApiError::Status { message, .. } => {
                self.schedule_retry();
                self.set_status(Status::Error { msg: message }, events);
            }
        }
    }

    fn schedule_retry(&mut self) {
        let now = (self.clock)();
        let delay = RETRY_DELAYS_MS[self.retry_index.min(RETRY_DELAYS_MS.len() - 1)];
        self.retry_at = Some(now.saturating_add(delay));
        self.retry_index = (self.retry_index + 1).min(RETRY_DELAYS_MS.len() - 1);
    }

    fn set_status(&mut self, status: Status, events: &mut Vec<SyncEvent>) {
        if self.status == status {
            return;
        }
        self.status = status.clone();
        events.push(SyncEvent::StatusChanged(status));
    }
}

fn auth_failure_message(err: &ApiError) -> String {
    match err {
        ApiError::Unauthorized => "用户名或密码错误".to_string(),
        ApiError::Network(message) => format!("无法连接服务器：{message}"),
        ApiError::Status { code, message, .. } => match code.as_deref() {
            Some("username_taken") => "用户名已被占用".to_string(),
            Some("invalid_credentials") => "用户名或密码错误".to_string(),
            _ => format!("登录失败：{message}"),
        },
    }
}

/// 中间页游标：本批**最后一条的 server_seq**（协议 §4，不能用 latest_seq）。
fn next_cursor(resp: &PullResponse, since: u64) -> Option<u64> {
    let cursor = resp.changes.last()?.server_seq?;
    if cursor <= since {
        return None;
    }
    Some(cursor)
}

/// 丢弃单条 payload 超过 [`MAX_PAYLOAD_BYTES`] 的变更（服务端会 413，且除非跳过否则
/// 每轮重推会陷入无限退避）。每轮至多发一条 [`SyncEvent::Warning`]，点名首个超限实体（§4）。
fn drop_oversize(changes: Vec<Change>, events: &mut Vec<SyncEvent>) -> Vec<Change> {
    let mut kept = Vec::with_capacity(changes.len());
    let mut first_offender: Option<(EntityKind, String, usize)> = None;
    for change in changes {
        let size = change
            .payload
            .as_ref()
            .and_then(|payload| serde_json::to_vec(payload).ok())
            .map(|bytes| bytes.len())
            .unwrap_or(0);
        if size > MAX_PAYLOAD_BYTES {
            if first_offender.is_none() {
                first_offender = Some((change.kind, change.id.clone(), size));
            }
            continue;
        }
        kept.push(change);
    }
    if let Some((kind, id, size)) = first_offender {
        events.push(SyncEvent::Warning(format!(
            "数据过大已跳过同步：{} {}（{size} 字节，超过 {MAX_PAYLOAD_BYTES} 字节上限）",
            kind.label_cn(),
            id,
        )));
    }
    kept
}

/// 按 ≤500 条 且 ≤3 MiB 分片，并在推送前填充 `device_id` 与出站戳收敛上限（§2.5 / §4）。
fn shard(changes: Vec<Change>, device_id: &str, last_server_time: Option<i64>) -> Vec<Vec<Change>> {
    let mut batches: Vec<Vec<Change>> = Vec::new();
    let mut current: Vec<Change> = Vec::new();
    // 序列化负载上限针对 `[...]` 数组，预留括号余量。
    let mut bytes: usize = 2;
    for mut change in changes {
        change.device_id = device_id.to_string();
        if let Some(server_time) = last_server_time {
            change.updated_at = outbound_cap(change.updated_at, server_time);
        }
        let len = serde_json::to_vec(&change)
            .map(|value| value.len())
            .unwrap_or(0)
            .saturating_add(1); // 逗号
        let would_exceed = !current.is_empty()
            && (current.len() + 1 > MAX_PUSH_CHANGES || bytes + len > MAX_PUSH_BYTES);
        if would_exceed {
            batches.push(std::mem::take(&mut current));
            bytes = 2;
        }
        current.push(change);
        bytes += len;
    }
    if !current.is_empty() {
        batches.push(current);
    }
    batches
}

#[cfg(test)]
mod tests;
