//! Gate-2 收敛证据（Phase 2b · 泳道 D2）。
//!
//! 每个测试在进程内 `127.0.0.1:0` + 临时 `DATA_DIR` 上启动**真实**服务端，并用
//! 「测试客户端」= `Database` + `SyncCore<ReqwestApi>` + 注入时钟 + 一个同步驱动器
//! （模拟 UI 线程职责：收集候选 / `apply_remote` / 回执 / 写游标）来驱动往返。
//!
//! 覆盖：两客户端双向收敛、首登三策略、clamp 交互（客户端时钟超前）、离线编辑后重连、
//! 删除 vs 编辑的复活语义（LWW），以及 3 客户端随机化收敛。

use std::collections::{BTreeMap, VecDeque};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use course_plan::model::{CheckIn, Course, CourseKind, Database, Memo, Reminder, Repeat};
use course_plan::sync::worker::ReqwestApi;
use course_plan::sync::{
    ApiError, FirstLoginPolicy, ProbeSummary, Status, SyncApi, SyncCommand, SyncCore,
    SyncCoreConfig, SyncEvent,
};
use course_plan_sync_proto::{
    AuthResponse, CLOCK_SKEW_ALLOWANCE_MS, Change, Credentials, HealthResponse, PullResponse,
    PushResponse,
};
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;

// ─────────────────────────────── 服务端 ───────────────────────────────

struct TestServer {
    base: String,
    _dir: tempfile::TempDir,
    handle: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

async fn start_server() -> TestServer {
    let dir = tempfile::tempdir().expect("tempdir");
    let config = course_plan_server::Config {
        port: 0,
        data_dir: dir.path().to_path_buf(),
        jwt_secret: "convergence-test-secret".to_string(),
    };
    let bind: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (addr, handle) = course_plan_server::spawn_server(&config, bind)
        .await
        .expect("spawn server");
    TestServer {
        base: format!("http://{addr}"),
        _dir: dir,
        handle,
    }
}

// ─────────────────────────────── 可离线 reqwest ───────────────────────────────

/// 包装 [`ReqwestApi`]，可在测试中翻转「离线」开关模拟网络中断。
struct Flaky {
    inner: ReqwestApi,
    online: Arc<AtomicBool>,
}

impl Flaky {
    fn new(online: Arc<AtomicBool>) -> Self {
        Self {
            inner: ReqwestApi::new().expect("reqwest client"),
            online,
        }
    }

    fn guard(&self) -> Result<(), ApiError> {
        if self.online.load(Ordering::Relaxed) {
            Ok(())
        } else {
            Err(ApiError::Network("simulated offline".into()))
        }
    }
}

impl SyncApi for Flaky {
    async fn login(
        &self,
        server_url: &str,
        credentials: &Credentials,
    ) -> Result<AuthResponse, ApiError> {
        self.guard()?;
        self.inner.login(server_url, credentials).await
    }

    async fn register(
        &self,
        server_url: &str,
        credentials: &Credentials,
    ) -> Result<AuthResponse, ApiError> {
        self.guard()?;
        self.inner.register(server_url, credentials).await
    }

    async fn push(
        &self,
        server_url: &str,
        token: &str,
        changes: &[Change],
    ) -> Result<PushResponse, ApiError> {
        self.guard()?;
        self.inner.push(server_url, token, changes).await
    }

    async fn pull(
        &self,
        server_url: &str,
        token: &str,
        since: u64,
    ) -> Result<PullResponse, ApiError> {
        self.guard()?;
        self.inner.pull(server_url, token, since).await
    }

    async fn health(&self, server_url: &str) -> Result<HealthResponse, ApiError> {
        self.guard()?;
        self.inner.health(server_url).await
    }
}

// ─────────────────────────────── 测试客户端 ───────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq)]
enum PolicyKind {
    /// 复刻 app：服务端空 → 推；本地空 → 拉；双方非空 → 合并。
    Auto,
    Merge,
    CloudWins,
    LocalWins,
}

struct Client {
    db: Database,
    core: SyncCore<Flaky>,
    online: Arc<AtomicBool>,
    policy: PolicyKind,
    server_url: String,
    last_status: Status,
    /// 若为 true：本轮持久化被模拟为失败（回执 `ok:false`），用后清零。
    fail_next_persist: bool,
    /// 若非空：在下一次 `NeedCandidates`（收集候选之后）注入一次本地编辑，
    /// 以模拟「轮次进行中本地又发生改动」，用后清零。
    inject_course: Option<(String, String)>,
}

fn now_real_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn unique_tag() -> usize {
    static COUNTER: AtomicUsize = AtomicUsize::new(1);
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

impl Client {
    fn new(server_url: &str, db: Database, policy: PolicyKind) -> Self {
        let online = Arc::new(AtomicBool::new(true));
        let api = Flaky::new(online.clone());
        let config = SyncCoreConfig {
            device_id: format!("dev-{}", unique_tag()),
            server_url: server_url.to_string(),
            ..Default::default()
        };
        let core = SyncCore::new(api, Box::new(now_real_ms), config);
        Self {
            db,
            core,
            online,
            policy,
            server_url: server_url.to_string(),
            last_status: Status::LoggedOut,
            fail_next_persist: false,
            inject_course: None,
        }
    }

    fn set_online(&self, online: bool) {
        self.online.store(online, Ordering::Relaxed);
    }

    fn last_seq(&self) -> u64 {
        self.core.last_seq()
    }

    async fn login(&mut self, username: &str, password: &str, register: bool) {
        self.run_commands(vec![SyncCommand::Login {
            server_url: self.server_url.clone(),
            username: username.to_string(),
            password: password.to_string(),
            register,
        }])
        .await;
    }

    async fn sync(&mut self) {
        self.run_commands(vec![SyncCommand::SyncNow]).await;
    }

    /// 模拟 UI 线程：处理事件（收集候选 / apply / 回执 / 写游标）。
    async fn run_commands(&mut self, commands: Vec<SyncCommand>) {
        let mut queue: VecDeque<SyncCommand> = commands.into();
        let mut guard = 0;
        while let Some(command) = queue.pop_front() {
            guard += 1;
            assert!(guard < 200_000, "驱动器未终止");
            let events = self.core.handle(command).await;
            for event in events {
                match event {
                    SyncEvent::RoundStarted { .. } => self.db.begin_round(0),
                    SyncEvent::NeedCandidates { round_id, full } => {
                        let since = if full { 0 } else { self.db.stamp_floor_ms };
                        let changes = self.db.all_changes(since);
                        queue.push_back(SyncCommand::Candidates { round_id, changes });
                        // 候选已收集 → 此刻注入「轮次进行中」的本地编辑（会记入
                        // round_min_mut_ts，必须在下一轮被补推）。
                        if let Some((id, name)) = self.inject_course.take() {
                            seed_course(&mut self.db, &id, &name);
                        }
                    }
                    SyncEvent::ApplyBatch {
                        round_id,
                        changes,
                        replace_all,
                        ..
                    } => {
                        if replace_all {
                            self.db = Database::default();
                        }
                        for change in &changes {
                            self.db.apply_remote(change);
                        }
                        let ok = !std::mem::take(&mut self.fail_next_persist);
                        queue.push_back(SyncCommand::AckRound { round_id, ok });
                    }
                    SyncEvent::ProbeResult { round_id, probe } => {
                        let policy = self.choose_policy(&probe);
                        queue.push_back(SyncCommand::FirstLoginPolicy { round_id, policy });
                    }
                    SyncEvent::RoundComplete {
                        round_id,
                        watermark_base_ms,
                        ..
                    } => {
                        if std::mem::take(&mut self.fail_next_persist) {
                            // 模拟 sync.json 落盘失败：游标/水位不推进，回 ok=false。
                            queue.push_back(SyncCommand::AckRound {
                                round_id,
                                ok: false,
                            });
                        } else {
                            let watermark = match self.db.round_min_mutation_ts() {
                                Some(min) => watermark_base_ms.min(min),
                                None => watermark_base_ms,
                            };
                            self.db.stamp_floor_ms = watermark;
                            queue.push_back(SyncCommand::AckRound { round_id, ok: true });
                        }
                    }
                    SyncEvent::AuthResult { .. } => {}
                    SyncEvent::Warning(_) => {}
                    SyncEvent::StatusChanged(status) => self.last_status = status,
                }
            }
        }
    }

    fn choose_policy(&mut self, probe: &ProbeSummary) -> FirstLoginPolicy {
        match self.policy {
            PolicyKind::Merge => FirstLoginPolicy::Merge,
            PolicyKind::CloudWins => FirstLoginPolicy::CloudWins,
            PolicyKind::LocalWins => {
                let base = now_real_ms().max(probe.server_time_ms.saturating_add(1));
                self.db.bump_all_stamps(base);
                self.db.stamp_floor_ms = base;
                FirstLoginPolicy::LocalWins {
                    local_ids: self.db.all_entity_ids(),
                }
            }
            PolicyKind::Auto => {
                if probe.all_ids.is_empty() {
                    FirstLoginPolicy::Merge
                } else if self.db.all_entity_ids().len() <= 1 {
                    FirstLoginPolicy::CloudWins
                } else {
                    FirstLoginPolicy::Merge
                }
            }
        }
    }
}

// ─────────────────────────────── 收敛断言 ───────────────────────────────

type Snapshot = BTreeMap<(String, String), (String, i64)>;

/// 逻辑收敛快照：由 `all_changes(0)` 取**存活**实体 → `(kind,id) -> (payload JSON, sync_ts)`。
///
/// 墓碑（本地行 / 有无该行 / 其残留 payload）**不是**协议收敛不变量：统一 apply（§2.4）
/// 只把墓碑落成 `deleted + ts`，保留本地原 payload；读取一律过滤墓碑。因此「两边都非存活」
/// 即等价，收敛断言只需存活集合（含 payload 与 sync_ts）一致——这也覆盖了
/// 「一边存活、另一边删除/缺失」的不收敛。
fn snapshot(db: &Database) -> Snapshot {
    db.all_changes(0)
        .into_iter()
        .filter(|change| !change.deleted)
        .map(|change| {
            let payload = change
                .payload
                .map(|value| value.to_string())
                .unwrap_or_default();
            (
                (change.kind.as_str().to_string(), change.id.clone()),
                (payload, change.updated_at),
            )
        })
        .collect()
}

fn snapshot_diff(a: &Snapshot, b: &Snapshot) -> String {
    let mut lines = Vec::new();
    for (key, value) in a {
        match b.get(key) {
            Some(other) if other == value => {}
            Some(other) => lines.push(format!("  差异 {key:?}: A={value:?} B={other:?}")),
            None => lines.push(format!("  仅 A 存活 {key:?}={value:?}")),
        }
    }
    for key in b.keys() {
        if !a.contains_key(key) {
            lines.push(format!("  仅 B 存活 {key:?}"));
        }
    }
    lines.truncate(12);
    lines.join("\n")
}

fn assert_converged(clients: &[Client], seed: Option<u64>) {
    let base = snapshot(&clients[0].db);
    for (index, client) in clients.iter().enumerate().skip(1) {
        let other = snapshot(&client.db);
        assert!(
            other == base,
            "客户端 0 与 {index} 未收敛{}:\n{}",
            seed.map(|s| format!(" (seed={s})")).unwrap_or_default(),
            snapshot_diff(&base, &other),
        );
    }
}

fn seed_course(db: &mut Database, id: &str, name: &str) {
    let mut course = Course::new(name, CourseKind::School);
    course.id = id.to_string();
    db.save_course(course);
}

async fn settle(clients: &mut [Client], rounds: usize) {
    for _ in 0..rounds {
        for client in clients.iter_mut() {
            client.sync().await;
        }
    }
}

// ─────────────────────────────── 确定性测试 ───────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_clients_bidirectional_convergence() {
    let server = start_server().await;
    let a = Client::new(&server.base, Database::default(), PolicyKind::Auto);
    let b = Client::new(&server.base, Database::default(), PolicyKind::Auto);
    let mut clients = vec![a, b];
    seed_course(&mut clients[0].db, "a1", "A数学");
    seed_course(&mut clients[1].db, "b1", "B语文");

    clients[0].login("family", "password123", true).await;
    clients[1].login("family", "password123", false).await;

    // 双方各自继续本地编辑：新增、更新、删除、打卡切换。
    seed_course(&mut clients[0].db, "a2", "A英语");
    seed_course(&mut clients[1].db, "b2", "B美术");
    let mut ci = CheckIn::new("阅读", Repeat::Daily);
    ci.id = "ci-a".into();
    clients[0].db.save_checkin(ci);
    let mut memo = Memo::new();
    memo.id = "m-b".into();
    memo.title = "B的备忘".into();
    clients[1].db.save_memo(memo);
    clients[1].db.delete_course("b2");

    settle(&mut clients, 6).await;
    assert_converged(&clients, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clamp_caps_far_future_stamps() {
    let server = start_server().await;
    let mut clients = [
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
    ];
    seed_course(&mut clients[0].db, "c1", "超前");
    // 模拟坏时钟：本地戳超前 1 小时。
    let ahead = now_real_ms() + 3_600_000;
    if let Some(course) = clients[0].db.courses.iter_mut().find(|c| c.id == "c1") {
        course.sync_ts = ahead;
    }
    clients[0].login("clock", "password123", true).await;

    // 本地保留原戳（出站收敛上限只影响 payload）。
    let local_ts = clients[0]
        .db
        .courses
        .iter()
        .find(|c| c.id == "c1")
        .unwrap()
        .sync_ts;
    assert_eq!(local_ts, ahead, "出站上限不应改动本地戳");

    // 另一台设备拉取：服务端存储已被 clamp 到 server_now + 300s 以内。
    clients[1].login("clock", "password123", false).await;
    let pulled = clients[1]
        .db
        .courses
        .iter()
        .find(|c| c.id == "c1")
        .expect("应拉到 c1");
    let ceiling = now_real_ms() + CLOCK_SKEW_ALLOWANCE_MS + 5_000;
    assert!(
        pulled.sync_ts <= ceiling,
        "服务端存储戳应被 clamp：{} > {}",
        pulled.sync_ts,
        ceiling
    );
    assert!(pulled.sync_ts > now_real_ms(), "clamp 到未来窗口内");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_login_merge_unions_both_sides() {
    let server = start_server().await;
    let mut clients = vec![
        Client::new(&server.base, Database::default(), PolicyKind::Merge),
        Client::new(&server.base, Database::default(), PolicyKind::Merge),
    ];
    seed_course(&mut clients[0].db, "a1", "A数学");
    clients[0].login("merge", "password123", true).await; // 服务端空 → 全量推

    seed_course(&mut clients[1].db, "b1", "B语文");
    clients[1].login("merge", "password123", false).await; // 双方非空 → 合并

    settle(&mut clients, 6).await;
    assert_converged(&clients, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_login_cloud_wins_discards_local() {
    let server = start_server().await;
    let mut clients = vec![
        Client::new(&server.base, Database::default(), PolicyKind::Merge),
        Client::new(&server.base, Database::default(), PolicyKind::CloudWins),
    ];
    seed_course(&mut clients[0].db, "a1", "A数学");
    clients[0].login("cloud", "password123", true).await;

    seed_course(&mut clients[1].db, "b1", "B本地将被丢弃");
    clients[1].login("cloud", "password123", false).await;

    settle(&mut clients, 6).await;
    assert_converged(&clients, None);
    assert!(clients[1].db.course("b1").is_none(), "b1 应被云端覆盖丢弃");
    assert!(
        !clients[1]
            .db
            .courses
            .iter()
            .any(|c| c.id == "b1" && c.deleted_at.is_some()),
        "云端覆盖本地不应产生本地墓碑"
    );
    assert!(clients[1].db.course("a1").is_some(), "应采用云端 a1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn first_login_local_wins_overrides_server() {
    let server = start_server().await;
    let mut clients = vec![
        Client::new(&server.base, Database::default(), PolicyKind::Merge),
        Client::new(&server.base, Database::default(), PolicyKind::LocalWins),
    ];
    seed_course(&mut clients[0].db, "a1", "A数学");
    let mut memo = Memo::new();
    memo.id = "a-memo".into();
    memo.title = "云端备忘".into();
    clients[0].db.save_memo(memo);
    clients[0].login("local", "password123", true).await;

    seed_course(&mut clients[1].db, "b1", "B本地");
    clients[1].login("local", "password123", false).await;

    settle(&mut clients, 8).await;
    assert_converged(&clients, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn offline_edit_then_reconnect_converges() {
    let server = start_server().await;
    let mut clients = vec![
        Client::new(&server.base, Database::default(), PolicyKind::Merge),
        Client::new(&server.base, Database::default(), PolicyKind::Merge),
    ];
    seed_course(&mut clients[0].db, "c1", "初始");
    clients[0].login("offline", "password123", true).await;
    clients[1].login("offline", "password123", false).await;
    settle(&mut clients, 4).await;

    // A 离线编辑，同步失败 → Offline。
    clients[0].set_online(false);
    let mut edited = clients[0].db.course("c1").unwrap().clone();
    edited.name = "A离线改".into();
    clients[0].db.save_course(edited);
    clients[0].sync().await;
    assert_eq!(
        clients[0].last_status,
        Status::Offline,
        "离线同步应报 Offline"
    );

    // B 在线编辑并推送。
    let mut edited_b = clients[1].db.course("c1").unwrap().clone();
    edited_b.name = "B在线改".into();
    clients[1].db.save_course(edited_b);
    clients[1].sync().await;

    // A 恢复网络后同步 → 收敛。
    clients[0].set_online(true);
    settle(&mut clients, 6).await;
    assert_converged(&clients, None);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_vs_edit_resurrects_via_lww() {
    let server = start_server().await;
    let mut clients = vec![
        Client::new(&server.base, Database::default(), PolicyKind::Merge),
        Client::new(&server.base, Database::default(), PolicyKind::Merge),
    ];
    seed_course(&mut clients[0].db, "c1", "初始");
    clients[0].login("resurrect", "password123", true).await;
    clients[1].login("resurrect", "password123", false).await;
    settle(&mut clients, 4).await;
    assert!(clients[1].db.course("c1").is_some());

    // A 删除（墓碑）。
    clients[0].db.delete_course("c1");
    clients[0].sync().await;

    // B 在不知道删除的情况下离线编辑，且戳更晚 → 复活（§9 已知 LWW 特性）。
    clients[1].set_online(false);
    let mut edited = clients[1].db.course("c1").unwrap().clone();
    edited.name = "B复活".into();
    clients[1].db.save_course(edited);
    clients[1].sync().await;
    clients[1].set_online(true);
    clients[1].sync().await;

    settle(&mut clients, 6).await;
    assert_converged(&clients, None);
    assert!(clients[1].db.course("c1").is_some(), "编辑应复活实体");
    assert_eq!(clients[1].db.course("c1").unwrap().name, "B复活");
}

// ─────────────────── Gate-2 复审修复回归（R2） ───────────────────

/// FIX 2：checkin 合并 bump 必须折进本轮水位（`round_min_mut_ts`），否则水位会
/// 把合并后的并集挡在下一轮之外。场景：A/B 共享打卡 → B 离线勾选一天 → A 编辑并
/// 同步 → B 重连（overridden 合并 bump）→ B **再跑一轮零本地改动** → A 应收到 B 的键。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn checkin_merge_bump_survives_watermark() {
    let server = start_server().await;
    let mut clients = [
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
    ];

    let mut ci = CheckIn::new("阅读", Repeat::Daily);
    ci.id = "ci".into();
    ci.history.insert("2026-10-01".into(), true);
    clients[0].db.checkins.push(ci);
    clients[0].login("mergebump", "password123", true).await; // 服务端空 → 推送
    clients[1].login("mergebump", "password123", false).await; // 拉取
    assert!(
        clients[1].db.checkins.iter().any(|c| c.id == "ci"),
        "B 应拉到共享打卡"
    );

    let day_b = course_plan::model::date_of(2);
    let day_a = course_plan::model::date_of(3);

    // B 离线勾选一天（本地编辑，同步失败）。
    clients[1].set_online(false);
    clients[1].db.set_checkin_done("ci", day_b, true);
    clients[1].sync().await;
    assert_eq!(clients[1].last_status, Status::Offline);

    // A 在线勾选另一天并同步（服务端版本更新）。
    clients[0].db.set_checkin_done("ci", day_a, true);
    clients[0].sync().await;

    // B 重连：第一次 sync 走 overridden 合并 bump；**再跑一轮零本地改动**应补推并集。
    clients[1].set_online(true);
    clients[1].sync().await;
    clients[1].sync().await;

    // A 再同步接收 B 勾选的那天。
    clients[0].sync().await;

    let a = clients[0]
        .db
        .checkins
        .iter()
        .find(|c| c.id == "ci")
        .unwrap();
    assert!(
        a.done_on(day_b),
        "A 应收到 B 勾选的那天（合并 bump 不得被水位挡掉）"
    );
    assert!(a.done_on(day_a), "A 自己的编辑也应保留");
}

/// FIX 1：持久化失败不得推进游标；随后重试幂等重放并收敛。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persist_failure_defers_cursor_then_recovers() {
    let server = start_server().await;
    let mut clients = vec![
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
    ];
    seed_course(&mut clients[0].db, "c1", "初始");
    clients[0].login("persistfail", "password123", true).await;
    clients[1].login("persistfail", "password123", false).await;
    settle(&mut clients, 4).await;

    let before = clients[1].last_seq();
    let mut edited = clients[1].db.course("c1").unwrap().clone();
    edited.name = "本地改".into();
    clients[1].db.save_course(edited);

    // 让本轮落盘失败（sync.json）→ 回执 ok=false。
    clients[1].fail_next_persist = true;
    clients[1].sync().await;

    assert_eq!(clients[1].last_seq(), before, "持久化失败不得推进游标");
    assert!(
        matches!(
            clients[1].last_status,
            Status::Error { .. } | Status::Offline
        ),
        "持久化失败应进入可重试状态：{:?}",
        clients[1].last_status
    );

    // 重试（手动触发，退避路径等价）→ 幂等重放并收敛。
    settle(&mut clients, 6).await;
    assert_converged(&clients, None);
    assert_eq!(
        clients[1].db.course("c1").unwrap().name,
        "本地改",
        "重试后本地编辑应落库并被对端看到"
    );
}

/// FIX「轮次进行中本地编辑」：从 `NeedCandidates` 之后注入的编辑必须记入
/// `round_min_mut_ts`，从而在下一轮被补推。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn edit_during_in_flight_round_is_pushed_next_round() {
    let server = start_server().await;
    let mut clients = vec![
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
    ];
    seed_course(&mut clients[0].db, "c1", "初始");
    clients[0].login("inflight", "password123", true).await;
    clients[1].login("inflight", "password123", false).await;
    settle(&mut clients, 4).await;

    // 本轮候选收集之后注入一次本地编辑（模拟轮内改动）。
    clients[0].inject_course = Some(("inflight-new".into(), "轮内新增".into()));
    clients[0].sync().await; // 轮内注入
    clients[0].sync().await; // 零本地改动的补推轮
    clients[1].sync().await; // 对端拉取

    assert!(
        clients[1].db.course("inflight-new").is_some(),
        "轮内注入的编辑应在下一轮被补推给对端"
    );
    assert_converged(&clients, None);
}

/// 平局（时间戳相同）不同内容：服务端持有者胜，双方收敛到服务端版本。
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn equal_timestamp_tie_converges_to_server_version() {
    let server = start_server().await;
    let mut clients = vec![
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
        Client::new(&server.base, Database::default(), PolicyKind::Auto),
    ];
    seed_course(&mut clients[0].db, "c1", "server-version");
    clients[0].login("tie", "password123", true).await;
    clients[1].login("tie", "password123", false).await;
    settle(&mut clients, 4).await;

    // B 在与服务端**相同**的时间戳上改内容（平局）。
    let server_ts = clients[1].db.course("c1").unwrap().sync_ts;
    {
        let course = clients[1]
            .db
            .courses
            .iter_mut()
            .find(|c| c.id == "c1")
            .unwrap();
        course.name = "client-version".into();
        course.sync_ts = server_ts;
    }
    clients[1].db.stamp_floor_ms = 0; // 确保本变更进入候选
    clients[1].sync().await; // 平局 → 服务端胜（overridden）
    clients[0].sync().await;

    settle(&mut clients, 3).await;
    assert_converged(&clients, None);
    assert_eq!(
        clients[0].db.course("c1").unwrap().name,
        "server-version",
        "平局应让位服务端版本"
    );
    assert_eq!(
        clients[1].db.course("c1").unwrap().name,
        "server-version",
        "平局应让位服务端版本"
    );
}

// ─────────────────────────────── 随机化收敛 ───────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn randomized_three_clients_converge() {
    for seed in [0x00C0_FFEE_u64, 0x0BAD_F00D_u64] {
        run_randomized(seed).await;
    }
}

async fn run_randomized(seed: u64) {
    let server = start_server().await;
    let mut rng = StdRng::seed_from_u64(seed);

    let mut clients: Vec<Client> = (0..3)
        .map(|i| {
            let mut db = Database::default();
            seed_course(&mut db, &format!("seed{i}"), &format!("初始{i}"));
            Client::new(&server.base, db, PolicyKind::Auto)
        })
        .collect();

    clients[0].login("random", "password123", true).await;
    for client in clients.iter_mut().skip(1) {
        client.login("random", "password123", false).await;
    }

    let ops = 300;
    for _ in 0..ops {
        let index = rng.random_range(0..clients.len());
        if rng.random_bool(0.05) {
            clients[index].set_online(false);
        }
        random_op(&mut clients[index].db, &mut rng, index);
        if rng.random_bool(0.35) {
            clients[index].sync().await;
        }
        if rng.random_bool(0.10) {
            clients[index].set_online(true);
        }
    }

    for client in clients.iter() {
        client.set_online(true);
    }

    // 有界轮次跑到静默（randomized 顺序）。
    for _ in 0..40 {
        let mut order: Vec<usize> = (0..clients.len()).collect();
        order.shuffle(&mut rng);
        for &i in &order {
            clients[i].sync().await;
        }
    }

    let base = snapshot(&clients[0].db);
    for (index, client) in clients.iter().enumerate().skip(1) {
        let other = snapshot(&client.db);
        assert!(
            other == base,
            "随机收敛失败 seed={seed}：客户端 0 vs {index}\n{}",
            snapshot_diff(&base, &other),
        );
    }
}

fn pick<T: Clone>(items: &[T], rng: &mut StdRng) -> Option<T> {
    if items.is_empty() {
        None
    } else {
        Some(items[rng.random_range(0..items.len())].clone())
    }
}

fn random_op(db: &mut Database, rng: &mut StdRng, tag: usize) {
    let today = course_plan::model::today();
    match rng.random_range(0..11) {
        0..=1 => {
            let name = format!("课{tag}-{}", rng.random_range(0..100_000));
            db.save_course(Course::new(name, CourseKind::School));
        }
        2 => {
            let mut ci = CheckIn::new(format!("打卡{tag}"), Repeat::Daily);
            ci.id = format!("ci{tag}-{}", rng.random_range(0..100_000));
            db.save_checkin(ci);
        }
        3 => db.save_memo(Memo::new()),
        4 => {
            let reminder = Reminder::new(
                format!("提醒{tag}-{}", rng.random_range(0..100_000)),
                "2026-01-01 09:00",
                Repeat::Once,
            );
            db.save_reminder(reminder);
        }
        5..=6 => {
            let ids: Vec<String> = db.live_courses().map(|c| c.id.clone()).collect();
            if let Some(id) = pick(&ids, rng) {
                let mut course = db.course(&id).unwrap().clone();
                course.name = format!("改{tag}-{}", rng.random_range(0..1000));
                db.save_course(course);
            }
        }
        7 => {
            let ids: Vec<String> = db.live_courses().map(|c| c.id.clone()).collect();
            if let Some(id) = pick(&ids, rng) {
                db.delete_course(&id);
            }
        }
        8 => {
            let ids: Vec<String> = db.live_checkins().map(|c| c.id.clone()).collect();
            if let Some(id) = pick(&ids, rng) {
                db.set_checkin_done(&id, today, rng.random_bool(0.5));
            }
        }
        9 => {
            let ids: Vec<String> = db.live_reminders().map(|r| r.id.clone()).collect();
            if let Some(id) = pick(&ids, rng) {
                db.set_reminder_done(&id, rng.random_bool(0.5));
            }
        }
        _ => {
            let ids: Vec<String> = db.live_memos().map(|m| m.id.clone()).collect();
            if let Some(id) = pick(&ids, rng) {
                db.delete_memo(&id);
            }
        }
    }
}
