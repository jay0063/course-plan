//! Phase 5 端到端同步泳道：证明**完整同步栈**（真实 HTTP + 真实客户端路径）。
//!
//! 与 `tests/convergence.rs`（用 `SyncCore` 直接驱动 + 假网络开关）不同，本测试
//! **优先走真实 worker 路径**：`SyncHandle::spawn` 启动专用 OS 线程 + 自带 tokio
//! runtime + reqwest(rustls)，再由本文件里的驱动器模拟 UI 线程（收集候选 /
//! `apply_remote` / 回执 / 写游标）。worker 只依赖 smol channel + tokio + reqwest，
//! 不需要 GPUI 上下文，因此可在集成测试里无头运行。
//!
//! 客户端数据一律使用内存中的 `course_plan::model::Database`（**不触碰**用户真实
//! 数据目录，也不调用 `course_plan::store`），服务端用 `tempfile::TempDir` 作为
//! `DATA_DIR`；因此本测试无需且不会设置 `COURSEPLAN_DATA_DIR`。
//!
//! ## 模式 1：进程内真实服务端（始终运行）
//! `127.0.0.1:0` + 临时 `DATA_DIR` + 测试 `JWT_SECRET`；两台设备（A/B）走真实
//! worker + 真实 HTTP：A 注册并造 course/memo/checkin → 同步 → 独立 pull 断言服务端
//! 状态；B 本地先有数据 → 首登 **Merge**（双方非空）→ 收敛；B 改一门、删一门 → 同步；
//! A 拉取 → 逐实体（payload / sync_ts / deleted）断言收敛。
//!
//! ## 模式 2：外部 / 容器化服务端（`COURSEPLAN_TEST_SERVER`，未设置则干净跳过）
//! 注册唯一用户 → 推一小批变更 → 拉回断言。可对接 `docker compose up`、部署实例，
//! 或本地 release 二进制。
//!
//! ### Docker compose 变体（本机未安装 Docker，故未在 CI/本机执行容器）
//! ```text
//! cp .env.example .env && $EDITOR .env      # 至少填入 JWT_SECRET
//! docker compose up -d --build              # 服务端监听 :8080（DB 卷 courseplan-data）
//! COURSEPLAN_TEST_SERVER=http://127.0.0.1:8080 \
//!   cargo test --test sync_e2e -- --nocapture
//! docker compose down
//! ```
//!
//! ### release 二进制变体（未装 Docker 时的本地等价路径；`scripts/e2e-server.sh` 已收录）
//! ```text
//! cargo build --release -p course-plan-server
//! DATA_DIR=$(mktemp -d) JWT_SECRET=test-secret PORT=18099 \
//!   ./target/release/course-plan-server &
//! COURSEPLAN_TEST_SERVER=http://127.0.0.1:18099 \
//!   cargo test --test sync_e2e -- --nocapture
//! kill %1
//! ```

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use course_plan::model::{CheckIn, Course, CourseKind, Database, Memo, Repeat};
use course_plan::sync::worker::{ReqwestApi, SyncHandle};
use course_plan::sync::{
    FirstLoginPolicy, Status, SyncApi, SyncCommand, SyncCoreConfig, SyncEvent,
};
use course_plan_sync_proto::{Change, Credentials};

/// worker 事件通道类型（native: `smol::channel::Receiver`）。
type EventRx = course_plan::gpui::base::async_util::Receiver<SyncEvent>;

// ─────────────────────────────── 进程内服务端 ───────────────────────────────

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
        jwt_secret: "sync-e2e-test-secret".to_string(),
    };
    let bind: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (addr, handle) = course_plan_server::spawn_server(&config, bind)
        .await
        .expect("spawn in-process server");
    TestServer {
        base: format!("http://{addr}"),
        _dir: dir,
        handle,
    }
}

// ─────────────────────────────── 真实 worker 设备 ───────────────────────────────

/// 一台设备：真实 `SyncHandle`（worker 线程 + reqwest）+ 内存数据库 + 驱动器。
struct Device {
    tag: String,
    handle: SyncHandle,
    events: EventRx,
    db: Database,
    server_url: String,
    last_status: Status,
}

fn unique_username(prefix: &str) -> String {
    static COUNTER: AtomicUsize = AtomicUsize::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    format!("{prefix}-{ms}-{n}")
}

impl Device {
    fn new(tag: &str, server_url: &str) -> Self {
        let config = SyncCoreConfig {
            device_id: format!("dev-{tag}-{}", unique_username("d")),
            server_url: server_url.to_string(),
            ..Default::default()
        };
        let mut handle = SyncHandle::spawn(config);
        let events = handle.take_events().expect("worker 事件通道只能取出一次");
        Self {
            tag: tag.to_string(),
            handle,
            events,
            db: Database::default(),
            server_url: server_url.to_string(),
            last_status: Status::LoggedOut,
        }
    }

    fn seed_course(&mut self, id: &str, name: &str) {
        let mut course = Course::new(name, CourseKind::School);
        course.id = id.to_string();
        self.db.save_course(course);
    }

    fn seed_memo(&mut self, id: &str, title: &str) {
        let mut memo = Memo::new();
        memo.id = id.to_string();
        memo.title = title.to_string();
        self.db.save_memo(memo);
    }

    fn seed_checkin(&mut self, id: &str, title: &str) {
        let mut checkin = CheckIn::new(title, Repeat::Daily);
        checkin.id = id.to_string();
        self.db.save_checkin(checkin);
    }

    /// 模拟 UI 线程：接收 worker 事件，收集候选 / apply / 回执，直到本轮回到 Idle。
    ///
    /// 「一轮结束」判据：先见到 `Syncing`，再见 `Idle`（首次登录会先探针→策略→合并轮，
    /// 全程在同一 pump 内被驱动完成）。
    async fn pump(&mut self) -> Result<(), String> {
        let mut saw_syncing = false;
        let deadline = Duration::from_secs(30);
        loop {
            let event = match tokio::time::timeout(deadline, self.events.recv()).await {
                Ok(Ok(event)) => event,
                Ok(Err(_)) => return Err(format!("[{}] worker 事件通道关闭", self.tag)),
                Err(_) => {
                    return Err(format!(
                        "[{}] 等待同步事件超时（{deadline:?}，末状态 {:?}）",
                        self.tag, self.last_status
                    ));
                }
            };
            match event {
                SyncEvent::RoundStarted { .. } => self.db.begin_round(0),
                SyncEvent::NeedCandidates { round_id, full } => {
                    let since = if full { 0 } else { self.db.stamp_floor_ms };
                    let changes = self.db.all_changes(since);
                    self.handle
                        .send(SyncCommand::Candidates { round_id, changes });
                }
                SyncEvent::ApplyBatch {
                    round_id,
                    changes,
                    replace_all,
                } => {
                    if replace_all {
                        self.db = Database::default();
                    }
                    for change in &changes {
                        self.db.apply_remote(change);
                    }
                    self.handle
                        .send(SyncCommand::AckRound { round_id, ok: true });
                }
                SyncEvent::ProbeResult { round_id, .. } => {
                    // 本测试固定首登策略为 Merge（A 空库、B 非空库时都适用）。
                    self.handle.send(SyncCommand::FirstLoginPolicy {
                        round_id,
                        policy: FirstLoginPolicy::Merge,
                    });
                }
                SyncEvent::RoundComplete {
                    round_id,
                    watermark_base_ms,
                    ..
                } => {
                    let watermark = match self.db.round_min_mutation_ts() {
                        Some(min) => watermark_base_ms.min(min),
                        None => watermark_base_ms,
                    };
                    self.db.stamp_floor_ms = watermark;
                    self.handle
                        .send(SyncCommand::AckRound { round_id, ok: true });
                }
                SyncEvent::AuthResult { ok, message, .. } => {
                    if !ok {
                        return Err(format!("[{}] 认证失败：{message}", self.tag));
                    }
                }
                SyncEvent::StatusChanged(status) => match &status {
                    Status::Syncing => saw_syncing = true,
                    Status::Idle { .. } if saw_syncing => {
                        self.last_status = status;
                        return Ok(());
                    }
                    Status::Offline if saw_syncing => {
                        self.last_status = status;
                        return Err(format!("[{}] 同步离线", self.tag));
                    }
                    Status::Error { msg } if saw_syncing => {
                        let msg = msg.clone();
                        self.last_status = status;
                        return Err(format!("[{}] 同步错误：{msg}", self.tag));
                    }
                    Status::AuthFailed => {
                        self.last_status = status;
                        return Err(format!("[{}] 凭据失效", self.tag));
                    }
                    _ => {}
                },
                SyncEvent::Warning(msg) => eprintln!("[{}] 同步告警：{msg}", self.tag),
            }
        }
    }

    async fn login(&mut self, username: &str, password: &str, register: bool) {
        self.handle.send(SyncCommand::Login {
            server_url: self.server_url.clone(),
            username: username.to_string(),
            password: password.to_string(),
            register,
        });
        self.pump().await.unwrap_or_else(|err| panic!("{err}"));
    }

    async fn sync(&mut self) {
        self.handle.send(SyncCommand::SyncNow);
        self.pump().await.unwrap_or_else(|err| panic!("{err}"));
    }
}

async fn settle(a: &mut Device, b: &mut Device, rounds: usize) {
    for _ in 0..rounds {
        a.sync().await;
        b.sync().await;
    }
}

// ─────────────────────────────── 服务端状态（独立 pull） ───────────────────────────────

/// 用独立的 `ReqwestApi`（不经过 worker/core 状态机）登录并全量拉取服务端实体。
///
/// 这是「assert server state via pull」——直接读服务端落库结果，验证推送确实到达。
async fn pull_server(server_url: &str, username: &str, password: &str) -> Vec<Change> {
    let api = ReqwestApi::new().expect("reqwest client");
    let creds = Credentials {
        username: username.to_string(),
        password: password.to_string(),
    };
    let auth = api
        .login(server_url, &creds)
        .await
        .expect("独立客户端登录服务端");
    let mut since = 0u64;
    let mut out: Vec<Change> = Vec::new();
    loop {
        let resp = api
            .pull(server_url, &auth.token, since)
            .await
            .expect("独立客户端拉取服务端");
        let has_more = resp.has_more;
        let next = resp.changes.last().and_then(|change| change.server_seq);
        out.extend(resp.changes);
        match (has_more, next) {
            (true, Some(cursor)) if cursor > since => since = cursor,
            (true, _) => panic!("独立 pull 游标未前进"),
            (false, _) => break,
        }
    }
    out
}

fn live_ids(changes: &[Change]) -> BTreeMap<(String, String), &Change> {
    changes
        .iter()
        .filter(|change| !change.deleted)
        .map(|change| {
            (
                (change.kind.as_str().to_string(), change.id.clone()),
                change,
            )
        })
        .collect()
}

// ─────────────────────────────── 收敛断言 ───────────────────────────────

type Snapshot = BTreeMap<(String, String), (String, i64)>;

/// 逻辑收敛快照：由 `all_changes(0)` 取**存活**实体 → `(kind,id) -> (payload JSON, sync_ts)`。
///
/// 墓碑不是协议收敛不变量（§2.4 统一 apply 保留本地原 payload）；「两边都非存活」即等价。
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
    lines.truncate(16);
    lines.join("\n")
}

fn assert_converged(devices: &[&Device], seed: &str) {
    let base = snapshot(&devices[0].db);
    for (index, device) in devices.iter().enumerate().skip(1) {
        let other = snapshot(&device.db);
        assert!(
            other == base,
            "seed={seed}: 设备 0({}) 与 {index}({}) 未收敛：\n{}",
            devices[0].tag,
            device.tag,
            snapshot_diff(&base, &other),
        );
    }
}

fn is_live(db: &Database, kind: &str, id: &str) -> bool {
    snapshot(db).contains_key(&(kind.to_string(), id.to_string()))
}

// ─────────────────────────────── 模式 1 ───────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mode1_in_process_server_full_stack_converges() {
    let server = start_server().await;
    let username = unique_username("e2e");
    let password = "password123";

    // ── A 注册 + 造 course/memo/checkin + 同步 ──
    let mut a = Device::new("A", &server.base);
    a.login(&username, password, true).await;
    a.seed_course("a-course", "A数学");
    a.seed_memo("a-memo", "A备忘");
    a.seed_checkin("a-checkin", "A打卡");
    a.sync().await;

    // 独立 pull 断言服务端已落库（不经过 A 的状态机）。
    let server_changes = pull_server(&server.base, &username, password).await;
    let live = live_ids(&server_changes);
    for (kind, id) in [
        ("course", "a-course"),
        ("memo", "a-memo"),
        ("checkin", "a-checkin"),
    ] {
        assert!(
            live.contains_key(&(kind.to_string(), id.to_string())),
            "服务端应已落库 {kind} {id}；实际存活 id：{:?}",
            live.keys().collect::<Vec<_>>()
        );
    }

    // ── B 本地先有数据 → 首登 Merge（双方非空）→ 收敛 ──
    let mut b = Device::new("B", &server.base);
    b.seed_course("b-course", "B语文");
    b.seed_memo("b-memo", "B备忘");
    b.login(&username, password, false).await; // 账号已存在：第二台设备登录

    settle(&mut a, &mut b, 6).await;
    assert_converged(&[&a, &b], "first-login-merge");
    assert!(
        is_live(&b.db, "course", "a-course"),
        "B 应通过首登 Merge 拉到 A 的课程"
    );
    assert!(
        is_live(&b.db, "course", "b-course"),
        "B 自己的本地课程应保留并上传"
    );
    assert!(is_live(&a.db, "course", "b-course"), "A 应拉到 B 的课程");

    // ── B 改一门、删一门 → 同步；A 拉取 → 逐实体收敛 ──
    let mut renamed = b.db.course("a-course").expect("B 应持有 a-course").clone();
    renamed.name = "A数学(改)".into();
    b.db.save_course(renamed);
    b.db.delete_course("b-course");
    b.sync().await;
    a.sync().await;
    settle(&mut a, &mut b, 4).await;

    assert_converged(&[&a, &b], "mutate-and-delete");

    // payload：改名应传播（payload 与 sync_ts 已由 snapshot 断言一致）。
    assert_eq!(
        a.db.course("a-course").expect("A 应持有 a-course").name,
        "A数学(改)",
        "B 的改名应传播到 A"
    );

    // deleted：B 删除的课程在双方均非存活，且各自保留墓碑。
    assert!(
        !is_live(&a.db, "course", "b-course"),
        "A 上 b-course 应删除"
    );
    assert!(
        !is_live(&b.db, "course", "b-course"),
        "B 上 b-course 应删除"
    );
    assert!(
        a.db.courses
            .iter()
            .any(|c| c.id == "b-course" && c.deleted_at.is_some()),
        "A 应保留 b-course 墓碑（删除语义经服务端传播）"
    );
    assert!(
        b.db.courses
            .iter()
            .any(|c| c.id == "b-course" && c.deleted_at.is_some()),
        "B 应保留 b-course 墓碑"
    );

    // sync_ts：改名后 a-course 的同步戳在两端一致且严格递增。
    let a_ts =
        a.db.courses
            .iter()
            .find(|c| c.id == "a-course")
            .unwrap()
            .sync_ts;
    let b_ts =
        b.db.courses
            .iter()
            .find(|c| c.id == "a-course")
            .unwrap()
            .sync_ts;
    assert_eq!(a_ts, b_ts, "a-course 的 sync_ts 应两端一致");
}

// ─────────────────────────────── 模式 2 ───────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn mode2_external_server_roundtrip() {
    let Ok(raw) = std::env::var("COURSEPLAN_TEST_SERVER") else {
        eprintln!("跳过模式 2：未设置 COURSEPLAN_TEST_SERVER（例如 http://127.0.0.1:18099）");
        return;
    };
    let url = raw.trim().trim_end_matches('/').to_string();
    if url.is_empty() {
        eprintln!("跳过模式 2：COURSEPLAN_TEST_SERVER 为空");
        return;
    }

    // 健康检查：确认外部服务端可达且契约一致。
    let api = ReqwestApi::new().expect("reqwest client");
    let health = api.health(&url).await.unwrap_or_else(|err| {
        panic!("外部服务端健康检查失败（{url}）：{err:?}");
    });
    assert_eq!(health.status, "ok", "健康检查状态应为 ok");
    eprintln!("模式 2 目标：{url}（health.status={}）", health.status);

    let username = unique_username("e2e-ext");
    let password = "password123";

    // 推一小批变更。
    let mut a = Device::new("ext-a", &url);
    a.login(&username, password, true).await;
    a.seed_course("ext-course", "外部课程");
    a.seed_memo("ext-memo", "外部备忘");
    a.sync().await;

    let server_changes = pull_server(&url, &username, password).await;
    let live = live_ids(&server_changes);
    assert!(
        live.contains_key(&("course".to_string(), "ext-course".to_string())),
        "外部服务端应已落库 ext-course"
    );
    assert!(
        live.contains_key(&("memo".to_string(), "ext-memo".to_string())),
        "外部服务端应已落库 ext-memo"
    );

    // 拉回断言：第二台设备登录后应看到这批变更。
    let mut b = Device::new("ext-b", &url);
    b.login(&username, password, false).await;
    b.sync().await;

    assert!(
        is_live(&b.db, "course", "ext-course"),
        "第二设备应拉到 ext-course"
    );
    assert!(
        is_live(&b.db, "memo", "ext-memo"),
        "第二设备应拉到 ext-memo"
    );
    assert_converged(&[&a, &b], "external-server-roundtrip");
    eprintln!("模式 2 通过：{url}");
}
