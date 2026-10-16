//! 同步 worker：专用 OS 线程 + 自带 tokio current-thread runtime + reqwest(rustls)。
//!
//! 严格遵循单写者契约（协议 §5.1）：worker **只做网络 + 轮次状态机**，不触碰
//! `Database`、不写文件。与 UI 通过 smol channel 通信：
//! - UI → worker：[`SyncCommand`]（[`SyncHandle::send`]）；
//! - worker → UI：[`SyncEvent`]（[`SyncHandle::take_events`] 取出接收端，由 App 消费）。

use std::sync::{Arc, Mutex};

use course_plan_sync_proto::{
    ApiError as ProtoApiError, AuthResponse, Change, Credentials, HealthResponse, PullResponse,
    PushRequest, PushResponse,
};

use super::core::{
    ApiError, Clock, Status, SyncApi, SyncCommand, SyncCore, SyncCoreConfig, SyncEvent,
};

/// GPUI 复用的跨平台 smol channel（native: smol::channel，wasm: async-channel）。
use crate::gpui::base::async_util as channel;

/// 同步句柄（App 持有）：命令发送端 + 事件接收端 + 状态快照。
pub struct SyncHandle {
    cmd_tx: Option<channel::Sender<SyncCommand>>,
    event_rx: Option<channel::Receiver<SyncEvent>>,
    shared: Arc<Mutex<Status>>,
    _thread: Option<std::thread::JoinHandle<()>>,
}

impl SyncHandle {
    /// 未启动的句柄（不连网、不起线程）。供无头渲染/注入式测试使用：
    /// 发送命令与取事件都是无操作。
    pub fn disabled() -> Self {
        Self {
            cmd_tx: None,
            event_rx: None,
            shared: Arc::new(Mutex::new(Status::LoggedOut)),
            _thread: None,
        }
    }

    /// 启动 worker 线程。`config` 来自 UI 读取的 `sync.json`。
    pub fn spawn(config: SyncCoreConfig) -> Self {
        let (cmd_tx, cmd_rx) = channel::unbounded();
        let (event_tx, event_rx) = channel::unbounded();
        let shared = Arc::new(Mutex::new(Status::LoggedOut));
        let shared_worker = shared.clone();
        let thread = std::thread::Builder::new()
            .name("course-plan-sync".to_string())
            .spawn(move || run(config, cmd_rx, event_tx, shared_worker))
            .ok();
        Self {
            cmd_tx: Some(cmd_tx),
            event_rx: Some(event_rx),
            shared,
            _thread: thread,
        }
    }

    /// 发送命令（非阻塞）。句柄未启动或 worker 已退出时静默丢弃。
    pub fn send(&self, command: SyncCommand) {
        if let Some(tx) = &self.cmd_tx {
            let _ = tx.try_send(command);
        }
    }

    /// 取出事件接收端（只能取一次；App 在启动时消费）。
    pub fn take_events(&mut self) -> Option<channel::Receiver<SyncEvent>> {
        self.event_rx.take()
    }

    /// 最新状态快照（worker 在发送 [`SyncEvent::StatusChanged`] 前更新）。
    pub fn status(&self) -> Status {
        self.shared
            .lock()
            .map(|guard| guard.clone())
            .unwrap_or(Status::LoggedOut)
    }
}

fn run(
    config: SyncCoreConfig,
    cmd_rx: channel::Receiver<SyncCommand>,
    event_tx: channel::Sender<SyncEvent>,
    shared: Arc<Mutex<Status>>,
) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(_) => return,
    };

    runtime.block_on(async move {
        let api = match ReqwestApi::new() {
            Ok(api) => api,
            Err(err) => {
                let status = Status::Error {
                    msg: format!("初始化网络客户端失败：{err}"),
                };
                publish(&event_tx, &shared, SyncEvent::StatusChanged(status));
                return;
            }
        };

        let clock: Clock = Box::new(system_now_ms);
        let mut core = SyncCore::new(api, clock, config);
        publish(&event_tx, &shared, SyncEvent::StatusChanged(core.status()));

        let mut ticker = tokio::time::interval(std::time::Duration::from_secs(1));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                command = cmd_rx.recv() => {
                    let Ok(command) = command else { break };
                    let events = core.handle(command).await;
                    for event in events {
                        publish(&event_tx, &shared, event);
                    }
                }
                _ = ticker.tick() => {
                    let events = core.tick().await;
                    for event in events {
                        publish(&event_tx, &shared, event);
                    }
                }
            }
        }
    });
}

fn publish(tx: &channel::Sender<SyncEvent>, shared: &Arc<Mutex<Status>>, event: SyncEvent) {
    if let SyncEvent::StatusChanged(status) = &event
        && let Ok(mut guard) = shared.lock()
    {
        *guard = status.clone();
    }
    let _ = tx.try_send(event);
}

fn system_now_ms() -> i64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}

// ─────────────────────────────── reqwest 实现 ───────────────────────────────

/// 基于 reqwest(rustls) 的 [`SyncApi`] 实现：15s 超时 + Bearer token。
pub struct ReqwestApi {
    client: reqwest::Client,
}

impl ReqwestApi {
    pub fn new() -> Result<Self, String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .map_err(|err| err.to_string())?;
        Ok(Self { client })
    }
}

impl SyncApi for ReqwestApi {
    async fn login(
        &self,
        server_url: &str,
        credentials: &Credentials,
    ) -> Result<AuthResponse, ApiError> {
        let response = self
            .client
            .post(format!("{server_url}/api/auth/login"))
            .json(credentials)
            .send()
            .await
            .map_err(network_error)?;
        parse_auth(response).await
    }

    async fn register(
        &self,
        server_url: &str,
        credentials: &Credentials,
    ) -> Result<AuthResponse, ApiError> {
        let response = self
            .client
            .post(format!("{server_url}/api/auth/register"))
            .json(credentials)
            .send()
            .await
            .map_err(network_error)?;
        parse_auth(response).await
    }

    async fn push(
        &self,
        server_url: &str,
        token: &str,
        changes: &[Change],
    ) -> Result<PushResponse, ApiError> {
        let response = self
            .client
            .post(format!("{server_url}/api/sync/push"))
            .bearer_auth(token)
            .json(&PushRequest {
                changes: changes.to_vec(),
            })
            .send()
            .await
            .map_err(network_error)?;
        let response = check_status(response).await?;
        response.json::<PushResponse>().await.map_err(network_error)
    }

    async fn pull(
        &self,
        server_url: &str,
        token: &str,
        since: u64,
    ) -> Result<PullResponse, ApiError> {
        let response = self
            .client
            .get(format!("{server_url}/api/sync/pull?since={since}"))
            .bearer_auth(token)
            .send()
            .await
            .map_err(network_error)?;
        let response = check_status(response).await?;
        response.json::<PullResponse>().await.map_err(network_error)
    }

    async fn health(&self, server_url: &str) -> Result<HealthResponse, ApiError> {
        let response = self
            .client
            .get(format!("{server_url}/api/health"))
            .send()
            .await
            .map_err(network_error)?;
        let response = check_status(response).await?;
        response
            .json::<HealthResponse>()
            .await
            .map_err(network_error)
    }
}

async fn parse_auth(response: reqwest::Response) -> Result<AuthResponse, ApiError> {
    let response = check_status(response).await?;
    response.json::<AuthResponse>().await.map_err(network_error)
}

fn network_error(err: reqwest::Error) -> ApiError {
    ApiError::Network(err.to_string())
}

/// 非 2xx → 分类错误；401/403 → `Unauthorized`，其它解析统一错误体。
async fn check_status(response: reqwest::Response) -> Result<reqwest::Response, ApiError> {
    if response.status().is_success() {
        return Ok(response);
    }
    let status = response.status().as_u16();
    if status == 401 || status == 403 {
        return Err(ApiError::Unauthorized);
    }
    let text = response.text().await.unwrap_or_default();
    let (code, message) = match serde_json::from_str::<ProtoApiError>(&text) {
        Ok(parsed) => (
            Some(parsed.error.code.as_str().to_string()),
            parsed.error.message,
        ),
        Err(_) => (None, text),
    };
    Err(ApiError::Status {
        status,
        code,
        message,
    })
}
