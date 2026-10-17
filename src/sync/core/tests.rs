//! `SyncCore` 确定性单测：注入时钟 + 假 [`SyncApi`]，不连网、不起线程。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};

use course_plan_sync_proto::{
    AuthResponse, Change, Credentials, EntityKind, HealthResponse, MAX_PUSH_BYTES,
    MAX_PUSH_CHANGES, PullResponse, PushAck, PushResponse, UserInfo,
};

use super::*;
use crate::model::{Course, CourseKind, Database};

// ─────────────────────────────── 假实现 ───────────────────────────────

#[derive(Clone, Default)]
struct FakeApi(Arc<Mutex<FakeInner>>);

#[derive(Default)]
struct FakeInner {
    auth: Option<Result<AuthResponse, ApiError>>,
    pull_script: VecDeque<Result<PullResponse, ApiError>>,
    push_script: VecDeque<Result<PushResponse, ApiError>>,
    pulls: Vec<u64>,
    pushes: Vec<Vec<Change>>,
    server_time: i64,
    default_latest: u64,
}

impl FakeApi {
    fn set_auth(&self, result: Result<AuthResponse, ApiError>) {
        self.0.lock().unwrap().auth = Some(result);
    }
    fn push_pull(&self, result: Result<PullResponse, ApiError>) {
        self.0.lock().unwrap().pull_script.push_back(result);
    }
    fn pulls(&self) -> Vec<u64> {
        self.0.lock().unwrap().pulls.clone()
    }
    fn pushes(&self) -> Vec<Vec<Change>> {
        self.0.lock().unwrap().pushes.clone()
    }
}

impl SyncApi for FakeApi {
    async fn login(
        &self,
        _server_url: &str,
        _credentials: &Credentials,
    ) -> Result<AuthResponse, ApiError> {
        self.0
            .lock()
            .unwrap()
            .auth
            .clone()
            .unwrap_or_else(|| Ok(auth_response()))
    }

    async fn register(
        &self,
        _server_url: &str,
        _credentials: &Credentials,
    ) -> Result<AuthResponse, ApiError> {
        self.login(_server_url, _credentials).await
    }

    async fn push(
        &self,
        _server_url: &str,
        _token: &str,
        changes: &[Change],
    ) -> Result<PushResponse, ApiError> {
        let mut guard = self.0.lock().unwrap();
        guard.pushes.push(changes.to_vec());
        if let Some(scripted) = guard.push_script.pop_front() {
            return scripted;
        }
        let applied = changes
            .iter()
            .map(|change| PushAck {
                kind: change.kind,
                id: change.id.clone(),
            })
            .collect();
        Ok(PushResponse {
            applied,
            overridden: Vec::new(),
            latest_seq: 0,
            server_time_ms: guard.server_time,
        })
    }

    async fn pull(
        &self,
        _server_url: &str,
        _token: &str,
        since: u64,
    ) -> Result<PullResponse, ApiError> {
        let mut guard = self.0.lock().unwrap();
        guard.pulls.push(since);
        if let Some(scripted) = guard.pull_script.pop_front() {
            return scripted;
        }
        Ok(PullResponse {
            changes: Vec::new(),
            latest_seq: guard.default_latest,
            has_more: false,
            server_time_ms: guard.server_time,
        })
    }

    async fn health(&self, _server_url: &str) -> Result<HealthResponse, ApiError> {
        Ok(HealthResponse {
            status: "ok".into(),
            version: "test".into(),
        })
    }
}

// ─────────────────────────────── 辅助 ───────────────────────────────

fn auth_response() -> AuthResponse {
    AuthResponse {
        token: "tok".into(),
        user: UserInfo {
            id: 1,
            username: "u".into(),
        },
    }
}

fn fake_clock(start: i64) -> (Arc<AtomicI64>, Clock) {
    let cell = Arc::new(AtomicI64::new(start));
    let reader = cell.clone();
    (cell, Box::new(move || reader.load(Ordering::Relaxed)))
}

fn config(token: &str, last_seq: u64) -> SyncCoreConfig {
    SyncCoreConfig {
        device_id: "dev-test".into(),
        server_url: "http://test".into(),
        username: "u".into(),
        user_id: Some(1),
        token: token.into(),
        last_seq,
    }
}

fn course_change(id: &str, ts: i64) -> Change {
    Change {
        kind: EntityKind::Course,
        id: id.into(),
        updated_at: ts,
        deleted: false,
        device_id: String::new(),
        payload: Some(serde_json::json!({ "id": id, "name": id })),
        server_seq: None,
    }
}

fn reply(
    event: &SyncEvent,
    candidates: &[Change],
    policy: &FirstLoginPolicy,
) -> Option<SyncCommand> {
    match event {
        SyncEvent::NeedCandidates { round_id, .. } => Some(SyncCommand::Candidates {
            round_id: *round_id,
            changes: candidates.to_vec(),
        }),
        SyncEvent::ApplyBatch { round_id, .. } => Some(SyncCommand::AckRound {
            round_id: *round_id,
            ok: true,
        }),
        SyncEvent::RoundComplete { round_id, .. } => Some(SyncCommand::AckRound {
            round_id: *round_id,
            ok: true,
        }),
        SyncEvent::ProbeResult { round_id, .. } => Some(SyncCommand::FirstLoginPolicy {
            round_id: *round_id,
            policy: policy.clone(),
        }),
        _ => None,
    }
}

/// 泵：按事件自动回执候选/ack/策略，直到命令队列与衍生回执都耗尽。
async fn pump(
    core: &mut SyncCore<FakeApi>,
    commands: Vec<SyncCommand>,
    candidates: &[Change],
    policy: &FirstLoginPolicy,
) -> Vec<SyncEvent> {
    let mut queue: VecDeque<SyncCommand> = commands.into();
    let mut collected = Vec::new();
    let mut guard = 0;
    while let Some(command) = queue.pop_front() {
        guard += 1;
        assert!(guard < 20_000, "pump 未终止");
        let events = core.handle(command).await;
        for event in &events {
            if let Some(next) = reply(event, candidates, policy) {
                queue.push_back(next);
            }
        }
        collected.extend(events);
    }
    collected
}

fn has_need_candidates(events: &[SyncEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, SyncEvent::NeedCandidates { .. }))
}

// ─────────────────────────────── 退避 ───────────────────────────────

#[tokio::test]
async fn backoff_delays_escalate_5_15_60_300() {
    let api = FakeApi::default();
    for _ in 0..4 {
        api.push_pull(Err(ApiError::Network("down".into())));
    }
    let (clock, clock_fn) = fake_clock(1_000_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    let delays = [5_000_i64, 15_000, 60_000, 300_000];
    let mut base = 1_000_000_i64;
    let mut events = core.handle(SyncCommand::SyncNow).await;
    for (attempt, delay) in delays.iter().enumerate() {
        let round_id = events
            .iter()
            .find_map(|e| match e {
                SyncEvent::NeedCandidates { round_id, .. } => Some(*round_id),
                _ => None,
            })
            .unwrap_or_else(|| panic!("尝试 {attempt} 应开始轮次"));
        // 回执候选 → 触发失败。
        core.handle(SyncCommand::Candidates {
            round_id,
            changes: Vec::new(),
        })
        .await;
        assert_eq!(core.status(), Status::Offline, "尝试 {attempt} 应离线");

        // 差 1ms 不应重试。
        let before = base + delay - 1;
        clock.store(before, Ordering::Relaxed);
        let early = core.tick().await;
        assert!(
            !has_need_candidates(&early),
            "尝试 {attempt}：{before} 不该重试"
        );

        // 到点应重试（tick 直接开启下一轮）。
        base += delay;
        clock.store(base, Ordering::Relaxed);
        events = core.tick().await;
        assert!(
            has_need_candidates(&events),
            "尝试 {attempt}：{base} 应重试"
        );
    }
    assert_eq!(api.pulls().len(), 4, "四次失败各发起一次 pull");
}

// ─────────────────────────────── 单飞 / 合并 ───────────────────────────────

#[tokio::test]
async fn single_flight_coalesces_triggers() {
    let api = FakeApi::default();
    let (_clock, clock_fn) = fake_clock(1_000);
    let mut core = SyncCore::new(api, clock_fn, config("t", 0));

    // 第一轮开始 → 等候选。
    let first = core.handle(SyncCommand::SyncNow).await;
    let round_id = first
        .iter()
        .find_map(|e| match e {
            SyncEvent::NeedCandidates { round_id, .. } => Some(*round_id),
            _ => None,
        })
        .unwrap();

    // 轮内再来两次触发：只置 pending，不新开轮。
    let during = core.handle(SyncCommand::SyncNow).await;
    assert!(!has_need_candidates(&during), "轮内触发不应新开轮");
    let during = core.handle(SyncCommand::LocalChanged).await;
    assert!(!has_need_candidates(&during), "轮内触发不应新开轮");

    // 完成本轮：pending 应立即补跑一轮，最终回到 Idle。
    let events = pump(
        &mut core,
        vec![SyncCommand::Candidates {
            round_id,
            changes: Vec::new(),
        }],
        &[],
        &FirstLoginPolicy::Merge,
    )
    .await;
    let rounds: Vec<u64> = events
        .iter()
        .filter_map(|e| match e {
            SyncEvent::NeedCandidates { round_id, .. } => Some(*round_id),
            _ => None,
        })
        .collect();
    assert_eq!(rounds.len(), 1, "pending 应补跑恰好一轮：{rounds:?}");
    assert_eq!(
        core.status(),
        Status::Idle {
            last_sync_ms: Some(1_000)
        }
    );
}

#[tokio::test]
async fn pending_trigger_reruns_after_round() {
    let api = FakeApi::default();
    let (_clock, clock_fn) = fake_clock(1_000);
    let mut core = SyncCore::new(api, clock_fn, config("t", 0));

    let first = core.handle(SyncCommand::SyncNow).await;
    let round_id = first
        .iter()
        .find_map(|e| match e {
            SyncEvent::NeedCandidates { round_id, .. } => Some(*round_id),
            _ => None,
        })
        .unwrap();
    // 轮内置 pending。
    core.handle(SyncCommand::LocalChanged).await;
    core.handle(SyncCommand::Candidates {
        round_id,
        changes: Vec::new(),
    })
    .await;
    // 完成本轮 → 立刻补跑（同一 ack 调用里应产出新的 NeedCandidates）。
    let done = core
        .handle(SyncCommand::AckRound { round_id, ok: true })
        .await;
    assert!(has_need_candidates(&done), "完成后应立即补跑 pending 轮次");
}

// ─────────────────────────────── 分片 ───────────────────────────────

#[tokio::test]
async fn batching_splits_by_count() {
    let api = FakeApi::default();
    let (_clock, clock_fn) = fake_clock(1_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    let candidates: Vec<Change> = (0..1201)
        .map(|i| course_change(&format!("c{i}"), (i + 1) as i64))
        .collect();
    let events = pump(
        &mut core,
        vec![SyncCommand::SyncNow],
        &candidates,
        &FirstLoginPolicy::Merge,
    )
    .await;
    assert!(!events.is_empty());

    let batches = api.pushes();
    let sizes: Vec<usize> = batches.iter().map(Vec::len).collect();
    assert_eq!(sizes, vec![500, 500, 201], "应按 500/批分片");
    assert!(
        batches.iter().all(|batch| batch.len() <= MAX_PUSH_CHANGES),
        "每批不超过上限"
    );
}

#[tokio::test]
async fn batching_splits_by_bytes() {
    let api = FakeApi::default();
    let (_clock, clock_fn) = fake_clock(1_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    // 每条 payload 均在 256 KiB 单条上限内，但总量超过 3 MiB → 必须按字节分片。
    let big = "x".repeat(200_000);
    let candidates: Vec<Change> = (0..20)
        .map(|i| Change {
            kind: EntityKind::Memo,
            id: format!("m{i}"),
            updated_at: (i + 1) as i64,
            deleted: false,
            device_id: String::new(),
            payload: Some(serde_json::json!({ "id": format!("m{i}"), "content": big })),
            server_seq: None,
        })
        .collect();

    pump(
        &mut core,
        vec![SyncCommand::SyncNow],
        &candidates,
        &FirstLoginPolicy::Merge,
    )
    .await;

    let batches = api.pushes();
    assert!(batches.len() >= 2, "大负载应按 3 MiB 分片");
    let flattened: usize = batches.iter().map(Vec::len).sum();
    assert_eq!(flattened, 20, "不丢变更");
    for batch in &batches {
        let size = serde_json::to_vec(batch).unwrap().len();
        assert!(size <= MAX_PUSH_BYTES, "分片负载超过 3 MiB：{size}");
    }
}

// ─────────────────────────────── 单条超限保护 ───────────────────────────────

#[tokio::test]
async fn oversize_payload_is_skipped_with_warning() {
    let api = FakeApi::default();
    let (_clock, clock_fn) = fake_clock(1_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    let oversized = Change {
        kind: EntityKind::Memo,
        id: "toobig".into(),
        updated_at: 1,
        deleted: false,
        device_id: String::new(),
        payload: Some(serde_json::json!({ "id": "toobig", "content": "x".repeat(300_000) })),
        server_seq: None,
    };
    let normal = course_change("small", 1);

    let events = pump(
        &mut core,
        vec![SyncCommand::SyncNow],
        &[oversized, normal],
        &FirstLoginPolicy::Merge,
    )
    .await;

    assert!(
        events
            .iter()
            .any(|event| matches!(event, SyncEvent::Warning(_))),
        "超限变更应触发一次告警"
    );
    let pushed: Vec<Change> = api.pushes().into_iter().flatten().collect();
    assert!(
        pushed.iter().all(|change| change.id != "toobig"),
        "超限变更应被跳过，避免服务端 413 无限退避"
    );
    assert!(
        pushed.iter().any(|change| change.id == "small"),
        "正常变更仍应被推送"
    );
}

/// 超限墓碑不得被跳过：剥掉 payload 后仍须推送，删除语义才得以传播。
#[tokio::test]
async fn oversize_tombstone_strips_payload_but_still_pushes() {
    let api = FakeApi::default();
    let (_clock, clock_fn) = fake_clock(1_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    let oversized_tombstone = Change {
        kind: EntityKind::Memo,
        id: "deleted-big".into(),
        updated_at: 2,
        deleted: true,
        device_id: String::new(),
        payload: Some(serde_json::json!({ "id": "deleted-big", "content": "x".repeat(300_000) })),
        server_seq: None,
    };

    pump(
        &mut core,
        vec![SyncCommand::SyncNow],
        &[oversized_tombstone],
        &FirstLoginPolicy::Merge,
    )
    .await;

    let pushed: Vec<Change> = api.pushes().into_iter().flatten().collect();
    let tombstone = pushed
        .iter()
        .find(|change| change.id == "deleted-big")
        .expect("超限墓碑应被推送，而非跳过");
    assert!(tombstone.deleted, "应保持墓碑语义");
    assert!(
        tombstone.payload.is_none(),
        "超限墓碑应剥离 payload（服务端接受 null payload）"
    );
}

// ─────────────────────────────── 游标规则 ───────────────────────────────

#[tokio::test]
async fn intermediate_pull_cursor_uses_last_server_seq() {
    let api = FakeApi::default();
    // 第一页：has_more=true，末条 server_seq=30，latest_seq=5000。
    let page: Vec<Change> = [10u64, 20, 30]
        .iter()
        .enumerate()
        .map(|(i, seq)| {
            let mut change = course_change(&format!("r{i}"), 100);
            change.server_seq = Some(*seq);
            change
        })
        .collect();
    api.push_pull(Ok(PullResponse {
        changes: page,
        latest_seq: 5000,
        has_more: true,
        server_time_ms: 999,
    }));
    api.push_pull(Ok(PullResponse {
        changes: Vec::new(),
        latest_seq: 5000,
        has_more: false,
        server_time_ms: 999,
    }));

    let (_clock, clock_fn) = fake_clock(1_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));
    pump(
        &mut core,
        vec![SyncCommand::SyncNow],
        &[],
        &FirstLoginPolicy::Merge,
    )
    .await;

    // 中间页游标必须是末条 server_seq(=30)，不是 latest_seq(=5000)。
    assert_eq!(api.pulls(), vec![0, 30], "中间页游标应为末条 server_seq");
    // 整轮结束游标推进到最终 latest_seq。
    assert_eq!(core.last_seq(), 5000, "整轮结束游标 = latest_seq");
}

// ─────────────────────────────── 401 / 离线重试 ───────────────────────────────

#[tokio::test]
async fn unauthorized_stops_auto_rounds() {
    let api = FakeApi::default();
    api.push_pull(Err(ApiError::Unauthorized));
    let (clock, clock_fn) = fake_clock(1_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    pump(
        &mut core,
        vec![SyncCommand::SyncNow],
        &[],
        &FirstLoginPolicy::Merge,
    )
    .await;
    assert_eq!(core.status(), Status::AuthFailed);

    // 即便过很久也不应自动重试。
    clock.store(1_000 + 600_000, Ordering::Relaxed);
    let events = core.tick().await;
    assert!(!has_need_candidates(&events), "AuthFailed 后应停止自动轮询");
    assert_eq!(api.pulls().len(), 1);
}

#[tokio::test]
async fn offline_retries_then_recovers() {
    let api = FakeApi::default();
    api.push_pull(Err(ApiError::Network("boom".into())));
    api.push_pull(Ok(PullResponse {
        changes: Vec::new(),
        latest_seq: 42,
        has_more: false,
        server_time_ms: 5_000,
    }));
    let (clock, clock_fn) = fake_clock(1_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    pump(
        &mut core,
        vec![SyncCommand::SyncNow],
        &[],
        &FirstLoginPolicy::Merge,
    )
    .await;
    assert_eq!(core.status(), Status::Offline);

    clock.store(1_000 + 5_000, Ordering::Relaxed);
    let retried = core.tick().await;
    let round_id = retried
        .iter()
        .find_map(|e| match e {
            SyncEvent::NeedCandidates { round_id, .. } => Some(*round_id),
            _ => None,
        })
        .expect("退避到点应重试");
    core.handle(SyncCommand::Candidates {
        round_id,
        changes: Vec::new(),
    })
    .await;
    core.handle(SyncCommand::AckRound { round_id, ok: true })
        .await;

    assert_eq!(
        core.status(),
        Status::Idle {
            last_sync_ms: Some(6_000)
        }
    );
    assert_eq!(core.last_seq(), 42);
}

// ─────────────────────────────── 防抖 ───────────────────────────────

#[tokio::test]
async fn local_changed_debounces_within_window() {
    let api = FakeApi::default();
    let (clock, clock_fn) = fake_clock(10_000);
    let mut core = SyncCore::new(api, clock_fn, config("t", 0));

    let events = core.handle(SyncCommand::LocalChanged).await;
    assert!(!has_need_candidates(&events), "防抖窗口内不应立即开轮");

    clock.store(10_000 + 2_999, Ordering::Relaxed);
    let early = core.tick().await;
    assert!(!has_need_candidates(&early), "差 1ms 不应开轮");

    clock.store(10_000 + 3_000, Ordering::Relaxed);
    let fired = core.tick().await;
    assert!(has_need_candidates(&fired), "到 3s 应开轮");
    // 完成本轮，回到 Idle。
    let round_id = fired
        .iter()
        .find_map(|e| match e {
            SyncEvent::NeedCandidates { round_id, .. } => Some(*round_id),
            _ => None,
        })
        .unwrap();
    pump(
        &mut core,
        vec![SyncCommand::Candidates {
            round_id,
            changes: Vec::new(),
        }],
        &[],
        &FirstLoginPolicy::Merge,
    )
    .await;
    assert_eq!(
        core.status(),
        Status::Idle {
            last_sync_ms: Some(13_000)
        }
    );

    // 两次 LocalChanged 在窗口内 → 合并（最后一次重置截止时间）。
    clock.store(20_000, Ordering::Relaxed);
    core.handle(SyncCommand::LocalChanged).await;
    clock.store(21_000, Ordering::Relaxed);
    core.handle(SyncCommand::LocalChanged).await;
    clock.store(23_500, Ordering::Relaxed);
    assert!(!has_need_candidates(&core.tick().await), "合并后截止为 24s");
    clock.store(24_000, Ordering::Relaxed);
    assert!(
        has_need_candidates(&core.tick().await),
        "合并窗口到点应开轮"
    );
}

// ─────────────────────────────── 首登 LocalWins 差集 ───────────────────────────────

#[tokio::test]
async fn local_wins_tombstones_server_only_ids() {
    let api = FakeApi::default();
    api.set_auth(Ok(auth_response()));
    // 探针：服务端存活 a、b、m。
    let probe_changes: Vec<Change> = vec![
        {
            let mut c = course_change("a", 1);
            c.server_seq = Some(1);
            c
        },
        {
            let mut c = course_change("b", 1);
            c.server_seq = Some(2);
            c
        },
        {
            let mut c = Change {
                kind: EntityKind::Memo,
                id: "m".into(),
                updated_at: 1,
                deleted: false,
                device_id: String::new(),
                payload: Some(serde_json::json!({ "id": "m" })),
                server_seq: Some(3),
            };
            c.server_seq = Some(3);
            c
        },
    ];
    api.push_pull(Ok(PullResponse {
        changes: probe_changes,
        latest_seq: 3,
        has_more: false,
        server_time_ms: 2_000,
    }));

    let (_clock, clock_fn) = fake_clock(10_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    // 登录 → 探针。
    let events = core
        .handle(SyncCommand::Login {
            server_url: "http://test".into(),
            username: "u".into(),
            password: "password123".into(),
            register: false,
        })
        .await;
    let round_id = events
        .iter()
        .find_map(|e| match e {
            SyncEvent::ProbeResult { round_id, probe } => {
                assert_eq!(probe.all_ids.len(), 3);
                Some(*round_id)
            }
            _ => None,
        })
        .expect("应有探针结果");

    // 本地只有 a。
    let local_ids = vec![(EntityKind::Course, "a".to_string())];
    let need = core
        .handle(SyncCommand::FirstLoginPolicy {
            round_id,
            policy: FirstLoginPolicy::LocalWins {
                local_ids,
                base_ms: 10_000,
            },
        })
        .await;
    let round_id = need
        .iter()
        .find_map(|e| match e {
            SyncEvent::NeedCandidates { round_id, full } => {
                assert!(*full, "LocalWins 应取全量候选");
                Some(*round_id)
            }
            _ => None,
        })
        .expect("应请求全量候选");
    core.handle(SyncCommand::Candidates {
        round_id,
        changes: vec![course_change("a", 10_000)],
    })
    .await;
    core.handle(SyncCommand::AckRound { round_id, ok: true })
        .await;

    let pushed: Vec<Change> = api.pushes().into_iter().flatten().collect();
    let tombstoned: Vec<(EntityKind, String)> = pushed
        .iter()
        .filter(|c| c.deleted)
        .map(|c| (c.kind, c.id.clone()))
        .collect();
    assert!(
        tombstoned.contains(&(EntityKind::Course, "b".to_string())),
        "应给服务端独有 course b 推墓碑：{tombstoned:?}"
    );
    assert!(
        tombstoned.contains(&(EntityKind::Memo, "m".to_string())),
        "应给服务端独有 memo m 推墓碑：{tombstoned:?}"
    );
    assert!(
        !tombstoned.iter().any(|(_, id)| id == "a"),
        "本地已有的 a 不应被墓碑"
    );
}

/// 服务端时钟领先本地时：LocalWins 墓碑必须以 UI 传入的 `base_ms`（=
/// `max(now, server_time + 1)`）重戳，而不是原始 `now`，否则会输给服务端既有实体。
#[tokio::test]
async fn local_wins_tombstones_use_bumped_base_when_server_ahead() {
    let api = FakeApi::default();
    // 探针：服务端存活 b（戳 20_000，约等于其时钟），客户端 now 仅 10_000。
    let probe_change = {
        let mut c = course_change("b", 20_000);
        c.server_seq = Some(1);
        c
    };
    api.push_pull(Ok(PullResponse {
        changes: vec![probe_change],
        latest_seq: 1,
        has_more: false,
        server_time_ms: 20_000,
    }));

    let (clock, clock_fn) = fake_clock(10_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    let events = core
        .handle(SyncCommand::Login {
            server_url: "http://test".into(),
            username: "u".into(),
            password: "password123".into(),
            register: false,
        })
        .await;
    let round_id = events
        .iter()
        .find_map(|e| match e {
            SyncEvent::ProbeResult { round_id, .. } => Some(*round_id),
            _ => None,
        })
        .expect("应有探针结果");

    // UI 侧基址 = max(now, server_time + 1) = 20_001（服务端时钟领先）。
    let server_time_ms = 20_000_i64;
    let base_ms = clock.load(Ordering::Relaxed).max(server_time_ms + 1);
    let need = core
        .handle(SyncCommand::FirstLoginPolicy {
            round_id,
            policy: FirstLoginPolicy::LocalWins {
                local_ids: Vec::new(),
                base_ms,
            },
        })
        .await;
    let round_id = need_round_id(&need);
    core.handle(SyncCommand::Candidates {
        round_id,
        changes: Vec::new(),
    })
    .await;
    core.handle(SyncCommand::AckRound { round_id, ok: true })
        .await;

    let pushed: Vec<Change> = api.pushes().into_iter().flatten().collect();
    let tombstone = pushed
        .iter()
        .find(|c| c.deleted && c.id == "b")
        .expect("应给服务端独有的 b 推墓碑");
    assert_eq!(
        tombstone.updated_at, base_ms,
        "墓碑须用 UI 的 bump 基址（{base_ms}），而非原始 now（10_000）"
    );
    assert!(
        tombstone.updated_at > 20_000,
        "墓碑戳须严格大于服务端既有戳，删除才会生效"
    );
}

// ─────────────────────────── 首登「稍后决定」───────────────────────────

fn has_round_started(events: &[SyncEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, SyncEvent::RoundStarted { .. }))
}

fn has_probe_result(events: &[SyncEvent]) -> bool {
    events
        .iter()
        .any(|event| matches!(event, SyncEvent::ProbeResult { .. }))
}

/// 登录 → 探针，返回探针轮次 id。
async fn login_to_probe(core: &mut SyncCore<FakeApi>) -> u64 {
    let events = core
        .handle(SyncCommand::Login {
            server_url: "http://test".into(),
            username: "u".into(),
            password: "password123".into(),
            register: false,
        })
        .await;
    events
        .iter()
        .find_map(|event| match event {
            SyncEvent::ProbeResult { round_id, .. } => Some(*round_id),
            _ => None,
        })
        .expect("登录后应有探针结果")
}

fn probe_response(latest_seq: u64) -> PullResponse {
    PullResponse {
        changes: Vec::new(),
        latest_seq,
        has_more: false,
        server_time_ms: 2_000,
    }
}

#[tokio::test]
async fn defer_first_login_returns_to_idle_and_keeps_cursors() {
    let api = FakeApi::default();
    api.set_auth(Ok(auth_response()));
    api.push_pull(Ok(probe_response(3)));
    let (_clock, clock_fn) = fake_clock(10_000);
    // 已持久化游标 7：首登暂缓不得重置它。
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 7));

    let round_id = login_to_probe(&mut core).await;
    // 探针更新了 watermark（server_time_ms），暂缓后应保持不变。
    assert_eq!(core.last_server_time_ms, Some(2_000));

    let events = core.handle(SyncCommand::DeferFirstLogin { round_id }).await;

    assert!(matches!(core.phase, Phase::Idle), "暂缓后应回到 Idle 相位");
    assert!(!core.pending, "暂缓应清空 pending");
    assert!(
        !matches!(core.status(), Status::Syncing),
        "暂缓后不得停留在 Syncing：{:?}",
        core.status()
    );
    assert_eq!(core.last_seq(), 7, "暂缓不得重置游标");
    assert_eq!(core.last_server_time_ms, Some(2_000), "暂缓不得改动水位");
    assert!(core.first_login_deferred, "应置首登暂缓标志");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, SyncEvent::StatusChanged(Status::Idle { .. }))),
        "应向 UI 报告 Idle"
    );
}

#[tokio::test]
async fn deferred_first_login_blocks_automatic_rounds() {
    let api = FakeApi::default();
    api.set_auth(Ok(auth_response()));
    api.push_pull(Ok(probe_response(3)));
    let (clock, clock_fn) = fake_clock(10_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    let round_id = login_to_probe(&mut core).await;
    core.handle(SyncCommand::DeferFirstLogin { round_id }).await;
    let pulls_after_defer = api.pulls().len();

    // 越过 5 分钟周期点：tick 不得开轮 / 探针。
    clock.store(10_000 + PERIODIC_MS + 1, Ordering::Relaxed);
    let ticked = core.tick().await;
    assert!(!has_round_started(&ticked), "暂缓期间周期 tick 不得开轮");
    assert!(!has_probe_result(&ticked), "暂缓期间周期 tick 不得探针");

    // LocalChanged → 防抖到点仍不得开轮。
    let now = clock.load(Ordering::Relaxed);
    core.handle(SyncCommand::LocalChanged).await;
    clock.store(now + DEBOUNCE_MS + 1, Ordering::Relaxed);
    let debounced = core.tick().await;
    assert!(
        !has_round_started(&debounced),
        "暂缓期间防抖到点不得开轮：{debounced:?}"
    );
    assert!(!has_probe_result(&debounced), "暂缓期间防抖不得探针");
    assert_eq!(
        api.pulls().len(),
        pulls_after_defer,
        "暂缓期间不得发起任何网络 pull"
    );
    assert!(matches!(core.phase, Phase::Idle), "仍应保持 Idle 相位");
    assert!(
        !matches!(core.status(), Status::Syncing),
        "仍不得处于 Syncing：{:?}",
        core.status()
    );
}

#[tokio::test]
async fn manual_sync_now_while_deferred_reruns_probe() {
    let api = FakeApi::default();
    api.set_auth(Ok(auth_response()));
    api.push_pull(Ok(probe_response(3)));
    api.push_pull(Ok(probe_response(9)));
    let (_clock, clock_fn) = fake_clock(10_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));

    let round_id = login_to_probe(&mut core).await;
    core.handle(SyncCommand::DeferFirstLogin { round_id }).await;
    assert!(core.first_login_deferred);

    let events = core.handle(SyncCommand::SyncNow).await;

    assert!(!core.first_login_deferred, "手动 SyncNow 应清除暂缓");
    let fresh = events
        .iter()
        .find_map(|event| match event {
            SyncEvent::ProbeResult { round_id, .. } => Some(*round_id),
            _ => None,
        })
        .expect("手动 SyncNow 应重新探针并发出 ProbeResult");
    assert_ne!(fresh, round_id, "应是一个新的探针轮次");
    assert!(
        matches!(core.phase, Phase::AwaitPolicy { round_id: rid, .. } if rid == fresh),
        "探针后应再次等待策略选择"
    );
    assert_eq!(api.pulls().len(), 2, "应重新拉取一次探针");
}

#[tokio::test]
async fn stale_defer_first_login_is_ignored() {
    let api = FakeApi::default();
    api.set_auth(Ok(auth_response()));
    api.push_pull(Ok(probe_response(3)));
    let (_clock, clock_fn) = fake_clock(10_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 5));

    let round_id = login_to_probe(&mut core).await;
    let events = core
        .handle(SyncCommand::DeferFirstLogin {
            round_id: round_id + 999,
        })
        .await;

    assert!(
        matches!(core.phase, Phase::AwaitPolicy { round_id: rid, .. } if rid == round_id),
        "过期 round_id 不得改变相位"
    );
    assert!(!core.first_login_deferred, "过期 round_id 不得置暂缓");
    assert!(
        matches!(core.status(), Status::Syncing),
        "过期 round_id 不得改变状态：{:?}",
        core.status()
    );
    assert_eq!(core.last_seq(), 5);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, SyncEvent::StatusChanged(Status::Idle { .. }))),
        "过期 round_id 不得发出 Idle 状态"
    );

    // 正确 round_id 仍应生效（证明仅暂缓未误置）。
    core.handle(SyncCommand::DeferFirstLogin { round_id }).await;
    assert!(core.first_login_deferred);
}

// ───────────────────────── FIX 1 / 崩溃重放（driver 级） ─────────────────────────

/// 构造一条可被 [`Database::apply_remote`] 解析的真实 course 实时变更。
fn course_live_change(id: &str, name: &str, ts: i64, seq: u64) -> Change {
    let mut course = Course::new(name, CourseKind::School);
    course.id = id.into();
    Change {
        kind: EntityKind::Course,
        id: id.into(),
        updated_at: ts,
        deleted: false,
        device_id: String::new(),
        payload: Some(serde_json::to_value(&course).unwrap()),
        server_seq: Some(seq),
    }
}

fn need_round_id(events: &[SyncEvent]) -> u64 {
    events
        .iter()
        .find_map(|event| match event {
            SyncEvent::NeedCandidates { round_id, .. } => Some(*round_id),
            _ => None,
        })
        .expect("应产出 NeedCandidates")
}

fn find_apply_batch(events: &[SyncEvent]) -> Vec<Change> {
    events
        .iter()
        .find_map(|event| match event {
            SyncEvent::ApplyBatch { changes, .. } => Some(changes.clone()),
            _ => None,
        })
        .expect("应产出 ApplyBatch")
}

fn snapshot_course(db: &Database, id: &str) -> Option<(String, i64)> {
    db.courses
        .iter()
        .find(|course| course.id == id)
        .map(|course| (course.name.clone(), course.sync_ts))
}

fn pull_one(change: Change, latest_seq: u64) -> PullResponse {
    PullResponse {
        changes: vec![change],
        latest_seq,
        has_more: false,
        server_time_ms: 5_000,
    }
}

/// FIX 1：最终 ack `ok=false`（sync.json 落盘失败）时游标不变、状态进入可重试；
/// 退避到点重试后幂等重放并推进游标。
#[tokio::test]
async fn persist_failure_keeps_cursor_and_replays_idempotently() {
    let api = FakeApi::default();
    let change = course_live_change("c1", "服务器", 111, 9);
    api.push_pull(Ok(pull_one(change.clone(), 9)));
    api.push_pull(Ok(pull_one(change, 9)));

    let (clock, clock_fn) = fake_clock(10_000);
    let mut core = SyncCore::new(api.clone(), clock_fn, config("t", 0));
    let mut db = Database::default();

    // 轮次 1：跑完 → ApplyBatch(data.json ok) → RoundComplete(sync.json 失败)。
    let need = core.handle(SyncCommand::SyncNow).await;
    let round_id = need_round_id(&need);
    let applied = core
        .handle(SyncCommand::Candidates {
            round_id,
            changes: Vec::new(),
        })
        .await;
    for change in &find_apply_batch(&applied) {
        db.apply_remote(change);
    }
    let snapshot = snapshot_course(&db, "c1");
    core.handle(SyncCommand::AckRound { round_id, ok: true })
        .await;
    let complete = core
        .handle(SyncCommand::AckRound {
            round_id,
            ok: false,
        })
        .await;

    assert_eq!(core.last_seq(), 0, "落盘失败不得推进游标");
    assert!(
        matches!(core.status(), Status::Error { .. }),
        "落盘失败应进入可重试状态：{:?}",
        core.status()
    );
    assert!(
        !complete
            .iter()
            .any(|event| matches!(event, SyncEvent::StatusChanged(Status::Idle { .. }))),
        "落盘失败不得翻成 Idle-success"
    );

    // 退避到点 → 重试；重放 apply 幂等。
    clock.store(15_000, Ordering::Relaxed);
    let retry = core.tick().await;
    let round_id = need_round_id(&retry);
    let applied = core
        .handle(SyncCommand::Candidates {
            round_id,
            changes: Vec::new(),
        })
        .await;
    for change in &find_apply_batch(&applied) {
        db.apply_remote(change);
    }
    assert_eq!(snapshot_course(&db, "c1"), snapshot, "重放 apply 应幂等");
    core.handle(SyncCommand::AckRound { round_id, ok: true })
        .await;
    core.handle(SyncCommand::AckRound { round_id, ok: true })
        .await;

    assert_eq!(core.last_seq(), 9, "重试成功后游标推进");
    assert!(matches!(core.status(), Status::Idle { .. }));
    assert_eq!(
        db.courses.iter().filter(|c| c.id == "c1").count(),
        1,
        "不应产生重复实体"
    );
}

/// 崩溃重放：跑到 RoundComplete 后跳过 final ack；用**已持久化游标**重建核心并重放同一
/// 脚本 → 重新拉取 + 幂等 apply（数据不变、无重复）。
#[tokio::test]
async fn crash_before_final_ack_replays_idempotently() {
    let api = FakeApi::default();
    let change = course_live_change("c1", "服务器", 111, 5);
    api.push_pull(Ok(pull_one(change.clone(), 5)));
    api.push_pull(Ok(pull_one(change, 5)));

    let (_c1, clock1) = fake_clock(10_000);
    let (_c2, clock2) = fake_clock(10_000);
    let mut db = Database::default();

    // 会话 1：跑到 RoundComplete 后崩溃（不 ack final）。
    let mut core1 = SyncCore::new(api.clone(), clock1, config("t", 0));
    let need = core1.handle(SyncCommand::SyncNow).await;
    let round_id = need_round_id(&need);
    let applied = core1
        .handle(SyncCommand::Candidates {
            round_id,
            changes: Vec::new(),
        })
        .await;
    for change in &find_apply_batch(&applied) {
        db.apply_remote(change);
    }
    let snapshot = snapshot_course(&db, "c1");
    core1
        .handle(SyncCommand::AckRound { round_id, ok: true })
        .await;
    assert_eq!(core1.last_seq(), 0, "崩溃前游标未推进");

    // 会话 2：用已持久化游标（0）重建核心，重放同一脚本。
    let mut core2 = SyncCore::new(api.clone(), clock2, config("t", 0));
    let need = core2.handle(SyncCommand::SyncNow).await;
    let round_id = need_round_id(&need);
    let applied = core2
        .handle(SyncCommand::Candidates {
            round_id,
            changes: Vec::new(),
        })
        .await;
    for change in &find_apply_batch(&applied) {
        db.apply_remote(change);
    }
    assert_eq!(snapshot_course(&db, "c1"), snapshot, "重放应不改变数据");
    core2
        .handle(SyncCommand::AckRound { round_id, ok: true })
        .await;
    core2
        .handle(SyncCommand::AckRound { round_id, ok: true })
        .await;

    assert_eq!(core2.last_seq(), 5, "重放后游标推进");
    assert_eq!(
        db.courses.iter().filter(|c| c.id == "c1").count(),
        1,
        "重放不应产生重复实体"
    );
}
