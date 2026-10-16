//! 服务端端点集成测试（Gate-1「脚本化 HTTP 客户端」证据）。
//!
//! 每个测试在临时 DATA_DIR 上启动真实 axum 服务端（`127.0.0.1:0`），
//! 并通过真实 HTTP 请求（reqwest）驱动完整的注册/登录/推送/拉取往返。

use std::collections::{HashMap, HashSet};

use course_plan_sync_proto::{
    AuthResponse, Change, Credentials, EntityKind, ErrorCode, HealthResponse, PullResponse,
    PushRequest, PushResponse, wins,
};
use reqwest::Client;
use serde_json::json;

/// 客户端实体视图：`(kind, id) -> (updated_at, deleted)`。
type View = HashMap<(String, String), (i64, bool)>;

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
    let dir = tempfile::tempdir().expect("failed to create tempdir");
    let config = course_plan_server::Config {
        port: 0,
        data_dir: dir.path().to_path_buf(),
        jwt_secret: "integration-test-secret".to_string(),
    };
    let bind: std::net::SocketAddr = "127.0.0.1:0".parse().unwrap();
    let (addr, handle) = course_plan_server::spawn_server(&config, bind)
        .await
        .expect("failed to spawn server");
    TestServer {
        base: format!("http://{addr}"),
        _dir: dir,
        handle,
    }
}

fn change(kind: EntityKind, id: &str, updated_at: i64, deleted: bool) -> Change {
    Change {
        kind,
        id: id.to_string(),
        updated_at,
        deleted,
        device_id: "test-device".to_string(),
        payload: if deleted {
            None
        } else {
            Some(json!({ "id": id }))
        },
        server_seq: None,
    }
}

fn with_device(mut change: Change, device_id: &str) -> Change {
    change.device_id = device_id.to_string();
    change
}

async fn register(client: &Client, base: &str, username: &str, password: &str) -> AuthResponse {
    let response = client
        .post(format!("{base}/api/auth/register"))
        .json(&Credentials {
            username: username.to_string(),
            password: password.to_string(),
        })
        .send()
        .await
        .expect("register request failed");
    assert_eq!(response.status(), 200, "register {username} failed");
    response.json().await.expect("register body")
}

async fn login_raw(
    client: &Client,
    base: &str,
    username: &str,
    password: &str,
) -> reqwest::Response {
    client
        .post(format!("{base}/api/auth/login"))
        .json(&Credentials {
            username: username.to_string(),
            password: password.to_string(),
        })
        .send()
        .await
        .expect("login request failed")
}

async fn push(client: &Client, base: &str, token: &str, changes: Vec<Change>) -> reqwest::Response {
    client
        .post(format!("{base}/api/sync/push"))
        .bearer_auth(token)
        .json(&PushRequest { changes })
        .send()
        .await
        .expect("push request failed")
}

async fn push_ok(client: &Client, base: &str, token: &str, changes: Vec<Change>) -> PushResponse {
    let response = push(client, base, token, changes).await;
    assert_eq!(response.status(), 200, "push failed");
    response.json().await.expect("push body")
}

async fn pull(client: &Client, base: &str, token: &str, since: u64) -> PullResponse {
    let response = client
        .get(format!("{base}/api/sync/pull?since={since}"))
        .bearer_auth(token)
        .send()
        .await
        .expect("pull request failed");
    assert_eq!(response.status(), 200, "pull failed");
    response.json().await.expect("pull body")
}

async fn error_code(response: reqwest::Response) -> (reqwest::StatusCode, ErrorCode) {
    let status = response.status();
    let body: serde_json::Value = response.json().await.expect("error body");
    let code: ErrorCode =
        serde_json::from_value(body["error"]["code"].clone()).expect("error code present");
    (status, code)
}

/// 客户端统一 apply（§2.4）：严格更新才覆盖。
fn apply_remote(view: &mut View, changes: &[Change]) {
    for change in changes {
        let key = (change.kind.as_str().to_string(), change.id.clone());
        match view.get(&key) {
            Some((existing_ts, _)) if !wins(change.updated_at, *existing_ts) => {}
            _ => {
                view.insert(key, (change.updated_at, change.deleted));
            }
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn health_endpoint_ok() {
    let server = start_server().await;
    let client = Client::new();

    let response = client
        .get(format!("{}/api/health", server.base))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 200);

    let body: HealthResponse = response.json().await.unwrap();
    assert_eq!(body.status, "ok");
    assert!(!body.version.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auth_register_login_duplicate_wrong_password() {
    let server = start_server().await;
    let client = Client::new();

    // register
    let auth = register(&client, &server.base, "xiaoman", "password123").await;
    assert!(auth.user.id > 0);
    assert_eq!(auth.user.username, "xiaoman");
    assert!(!auth.token.is_empty());

    // login with same credentials
    let response = login_raw(&client, &server.base, "xiaoman", "password123").await;
    assert_eq!(response.status(), 200);
    let login: AuthResponse = response.json().await.unwrap();
    assert_eq!(login.user.id, auth.user.id);
    assert!(!login.token.is_empty());

    // duplicate register (case-insensitive uniqueness) → 409 username_taken
    let response = client
        .post(format!("{}/api/auth/register", server.base))
        .json(&Credentials {
            username: "XiaoMan".to_string(),
            password: "password123".to_string(),
        })
        .send()
        .await
        .unwrap();
    let (status, code) = error_code(response).await;
    assert_eq!(status, 409);
    assert_eq!(code, ErrorCode::UsernameTaken);

    // wrong password → 401 invalid_credentials
    let response = login_raw(&client, &server.base, "xiaoman", "wrong-password").await;
    let (status, code) = error_code(response).await;
    assert_eq!(status, 401);
    assert_eq!(code, ErrorCode::InvalidCredentials);

    // unknown user → 401 invalid_credentials (uniform)
    let response = login_raw(&client, &server.base, "nobody", "password123").await;
    let (status, code) = error_code(response).await;
    assert_eq!(status, 401);
    assert_eq!(code, ErrorCode::InvalidCredentials);

    // username too short → 400 invalid_request
    let response = client
        .post(format!("{}/api/auth/register", server.base))
        .json(&Credentials {
            username: "a".to_string(),
            password: "password123".to_string(),
        })
        .send()
        .await
        .unwrap();
    let (status, code) = error_code(response).await;
    assert_eq!(status, 400);
    assert_eq!(code, ErrorCode::InvalidRequest);

    // username with whitespace → 400 invalid_request
    let response = client
        .post(format!("{}/api/auth/register", server.base))
        .json(&Credentials {
            username: "bad name".to_string(),
            password: "password123".to_string(),
        })
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), 400);

    // password too short → 400 invalid_request
    let response = client
        .post(format!("{}/api/auth/register", server.base))
        .json(&Credentials {
            username: "shortpass".to_string(),
            password: "short".to_string(),
        })
        .send()
        .await
        .unwrap();
    let (status, code) = error_code(response).await;
    assert_eq!(status, 400);
    assert_eq!(code, ErrorCode::InvalidRequest);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_or_invalid_token_is_unauthorized() {
    let server = start_server().await;
    let client = Client::new();

    // no Authorization header
    let response = client
        .get(format!("{}/api/sync/pull?since=0", server.base))
        .send()
        .await
        .unwrap();
    let (status, code) = error_code(response).await;
    assert_eq!(status, 401);
    assert_eq!(code, ErrorCode::Unauthorized);

    // malformed / invalid token
    let response = client
        .get(format!("{}/api/sync/pull?since=0", server.base))
        .bearer_auth("not-a-real-jwt")
        .send()
        .await
        .unwrap();
    let (status, code) = error_code(response).await;
    assert_eq!(status, 401);
    assert_eq!(code, ErrorCode::Unauthorized);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn push_pull_roundtrip_and_cursor_advance() {
    let server = start_server().await;
    let client = Client::new();
    let auth = register(&client, &server.base, "roundtrip", "password123").await;
    let token = auth.token;

    let first = push_ok(
        &client,
        &server.base,
        &token,
        vec![change(EntityKind::Course, "c1", 1_000, false)],
    )
    .await;
    assert_eq!(first.applied.len(), 1);
    assert_eq!(first.applied[0].id, "c1");
    assert!(first.overridden.is_empty());
    assert_eq!(first.latest_seq, 1);

    let second = push_ok(
        &client,
        &server.base,
        &token,
        vec![change(EntityKind::Memo, "m1", 1_001, false)],
    )
    .await;
    assert_eq!(second.latest_seq, 2);

    let page = pull(&client, &server.base, &token, 0).await;
    assert_eq!(page.changes.len(), 2);
    assert_eq!(page.latest_seq, 2);
    assert!(!page.has_more);
    assert_eq!(page.changes[0].id, "c1");
    assert_eq!(page.changes[0].server_seq, Some(1));
    assert_eq!(page.changes[1].id, "m1");
    assert_eq!(page.changes[1].server_seq, Some(2));

    // cursor at latest → nothing new
    let empty = pull(&client, &server.base, &token, page.latest_seq).await;
    assert!(empty.changes.is_empty());
    assert_eq!(empty.latest_seq, 2);
    assert!(!empty.has_more);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pull_paginates_over_1000_changes() {
    let server = start_server().await;
    let client = Client::new();
    let auth = register(&client, &server.base, "pagination", "password123").await;
    let token = auth.token;

    let total = 1001usize;
    let mut pushed = 0usize;
    while pushed < total {
        let end = (pushed + 500).min(total);
        let batch: Vec<Change> = (pushed..end)
            .map(|i| change(EntityKind::Memo, &format!("m{i}"), 1_000 + i as i64, false))
            .collect();
        let response = push_ok(&client, &server.base, &token, batch).await;
        assert!(!response.applied.is_empty());
        pushed = end;
    }

    // first page is capped at 1000 with has_more
    let first = pull(&client, &server.base, &token, 0).await;
    assert_eq!(first.changes.len(), 1000);
    assert!(first.has_more);
    assert_eq!(first.latest_seq, total as u64);

    // client pages through every change exactly once
    let mut seen: HashSet<String> = HashSet::new();
    let mut since = 0u64;
    let mut batches = 0;
    loop {
        let page = pull(&client, &server.base, &token, since).await;
        for item in &page.changes {
            assert!(seen.insert(item.id.clone()), "duplicate id {}", item.id);
        }
        // 游标推进：has_more 时应取「本批最后一条 server_seq」（协议 §4：
        // 仅在整个循环结束后才将 last_seq 设为最终 latest_seq）。
        since = match page.changes.last() {
            Some(last) => last.server_seq.expect("server_seq present"),
            None => page.latest_seq,
        };
        batches += 1;
        assert!(batches <= 10, "pagination did not terminate");
        if !page.has_more {
            assert_eq!(page.latest_seq, total as u64);
            break;
        }
    }
    assert_eq!(seen.len(), total);
    assert_eq!(since, total as u64);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn lww_newer_wins_older_and_tie_overridden() {
    let server = start_server().await;
    let client = Client::new();
    let auth = register(&client, &server.base, "lww", "password123").await;
    let token = auth.token;

    // baseline applied
    let first = push_ok(
        &client,
        &server.base,
        &token,
        vec![change(EntityKind::Course, "e", 1_000, false)],
    )
    .await;
    assert_eq!(first.applied.len(), 1);
    assert_eq!(first.latest_seq, 1);

    // equal timestamp → overridden (server wins tie)
    let tie = push_ok(
        &client,
        &server.base,
        &token,
        vec![change(EntityKind::Course, "e", 1_000, false)],
    )
    .await;
    assert!(tie.applied.is_empty());
    assert_eq!(tie.overridden.len(), 1);
    assert_eq!(tie.overridden[0].updated_at, 1_000);
    assert_eq!(tie.overridden[0].server_seq, Some(1));

    // older → overridden with authoritative version
    let older = push_ok(
        &client,
        &server.base,
        &token,
        vec![change(EntityKind::Course, "e", 500, false)],
    )
    .await;
    assert_eq!(older.overridden.len(), 1);
    assert_eq!(older.overridden[0].updated_at, 1_000);
    assert_eq!(older.overridden[0].server_seq, Some(1));

    // newer → applied
    let newer = push_ok(
        &client,
        &server.base,
        &token,
        vec![change(EntityKind::Course, "e", 2_000, false)],
    )
    .await;
    assert_eq!(newer.applied.len(), 1);

    let page = pull(&client, &server.base, &token, 0).await;
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].updated_at, 2_000);
    assert_eq!(page.changes[0].server_seq, Some(2));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn clamp_caps_far_future_timestamp() {
    let server = start_server().await;
    let client = Client::new();
    let auth = register(&client, &server.base, "clamp", "password123").await;
    let token = auth.token;

    // ~year 2096, far beyond now + 300s
    let far_future = 4_000_000_000_000_000_i64;
    let response = push_ok(
        &client,
        &server.base,
        &token,
        vec![change(EntityKind::Event, "e", far_future, false)],
    )
    .await;
    assert_eq!(response.applied.len(), 1);

    let expected = response.server_time_ms + 300_000;
    assert!(expected < far_future);

    let page = pull(&client, &server.base, &token, 0).await;
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].updated_at, expected);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn tombstone_propagates_to_other_client() {
    let server = start_server().await;
    let client = Client::new();
    let auth = register(&client, &server.base, "tombstone", "password123").await;
    let token = auth.token.clone();

    // client A creates then deletes the entity
    let created = with_device(change(EntityKind::Course, "c", 1_000, false), "A");
    let applied = push_ok(&client, &server.base, &token, vec![created]).await;
    assert_eq!(applied.applied.len(), 1);

    let tombstoned = with_device(change(EntityKind::Course, "c", 2_000, true), "A");
    let applied = push_ok(&client, &server.base, &token, vec![tombstoned]).await;
    assert_eq!(applied.applied.len(), 1);
    assert_eq!(applied.latest_seq, 2);

    // client B (second device on same account) pulls and sees the tombstone
    let b_login: AuthResponse = login_raw(&client, &server.base, "tombstone", "password123")
        .await
        .json()
        .await
        .unwrap();
    let page = pull(&client, &server.base, &b_login.token, 0).await;
    assert_eq!(page.changes.len(), 1);
    let item = &page.changes[0];
    assert_eq!(item.id, "c");
    assert!(item.deleted);
    assert_eq!(item.server_seq, Some(2));
    assert_eq!(item.device_id, "A");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn accounts_are_isolated() {
    let server = start_server().await;
    let client = Client::new();

    let alice = register(&client, &server.base, "alice", "password123").await;
    let bob = register(&client, &server.base, "bob", "password123").await;

    push_ok(
        &client,
        &server.base,
        &alice.token,
        vec![change(EntityKind::Course, "a1", 1_000, false)],
    )
    .await;
    push_ok(
        &client,
        &server.base,
        &bob.token,
        vec![change(EntityKind::Course, "b1", 1_000, false)],
    )
    .await;

    let alice_page = pull(&client, &server.base, &alice.token, 0).await;
    assert_eq!(alice_page.changes.len(), 1);
    assert_eq!(alice_page.changes[0].id, "a1");

    let bob_page = pull(&client, &server.base, &bob.token, 0).await;
    assert_eq!(bob_page.changes.len(), 1);
    assert_eq!(bob_page.changes[0].id, "b1");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn push_batch_and_payload_limits() {
    let server = start_server().await;
    let client = Client::new();
    let auth = register(&client, &server.base, "limits", "password123").await;
    let token = auth.token;

    // 501 changes → 400 invalid_request (batch size)
    let too_many: Vec<Change> = (0..501)
        .map(|i| change(EntityKind::Memo, &format!("m{i}"), 1_000, false))
        .collect();
    let response = push(&client, &server.base, &token, too_many).await;
    let (status, code) = error_code(response).await;
    assert_eq!(status, 400);
    assert_eq!(code, ErrorCode::InvalidRequest);

    // single oversize payload (>256 KiB) → 413 payload_too_large
    let mut oversize = change(EntityKind::Memo, "big", 1_000, false);
    oversize.payload = Some(serde_json::Value::String("x".repeat(300 * 1024)));
    let response = push(&client, &server.base, &token, vec![oversize]).await;
    let (status, code) = error_code(response).await;
    assert_eq!(status, 413);
    assert_eq!(code, ErrorCode::PayloadTooLarge);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn push_rejects_live_entity_without_payload() {
    let server = start_server().await;
    let client = Client::new();
    let auth = register(&client, &server.base, "no-payload", "password123").await;
    let token = auth.token;

    // 活跃实体缺 payload → 整批 400 invalid_request
    let mut live_without_payload = change(EntityKind::Course, "c1", 1_000, false);
    live_without_payload.payload = None;
    let response = push(&client, &server.base, &token, vec![live_without_payload]).await;
    let (status, code) = error_code(response).await;
    assert_eq!(status, 400);
    assert_eq!(code, ErrorCode::InvalidRequest);

    // 被拒批次原子地不落库
    assert!(
        pull(&client, &server.base, &token, 0)
            .await
            .changes
            .is_empty(),
        "rejected batch must not persist anything"
    );

    // tombstone（deleted=true）无 payload 仍然合法
    let tombstone = change(EntityKind::Course, "c2", 1_000, true);
    let ok = push_ok(&client, &server.base, &token, vec![tombstone]).await;
    assert_eq!(ok.applied.len(), 1);

    let page = pull(&client, &server.base, &token, 0).await;
    assert_eq!(page.changes.len(), 1);
    assert_eq!(page.changes[0].id, "c2");
    assert!(page.changes[0].deleted);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pull_rejects_invalid_since() {
    let server = start_server().await;
    let client = Client::new();
    let auth = register(&client, &server.base, "bad-since", "password123").await;

    // 非 u64 / u64 溢出 / 负值 → 统一 400 invalid_request（JSON 错误体）
    for raw in ["abc", "99999999999999999999", "-1"] {
        let response = client
            .get(format!("{}/api/sync/pull?since={raw}", server.base))
            .bearer_auth(&auth.token)
            .send()
            .await
            .expect("pull request failed");
        let (status, code) = error_code(response).await;
        assert_eq!(status, 400, "since={raw} must be rejected");
        assert_eq!(code, ErrorCode::InvalidRequest, "since={raw}");
    }

    // 缺省 since → 0（合法）
    let response = client
        .get(format!("{}/api/sync/pull", server.base))
        .bearer_auth(&auth.token)
        .send()
        .await
        .expect("pull request failed");
    assert_eq!(response.status(), 200);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn two_scripted_clients_converge() {
    let server = start_server().await;
    let client = Client::new();
    let account = register(&client, &server.base, "converge", "password123").await;
    let token = account.token;

    // client A pushes three entities
    let a_changes = vec![
        with_device(change(EntityKind::Course, "e1", 1_000, false), "A"),
        with_device(change(EntityKind::Course, "e2", 1_001, false), "A"),
        with_device(change(EntityKind::Memo, "e3", 1_002, false), "A"),
    ];
    let pushed = push_ok(&client, &server.base, &token, a_changes.clone()).await;
    assert_eq!(pushed.applied.len(), 3);
    let a_cursor = pushed.latest_seq;

    let mut view_a: View = HashMap::new();
    apply_remote(&mut view_a, &a_changes);

    // client B pulls everything A wrote
    let page = pull(&client, &server.base, &token, 0).await;
    assert_eq!(page.changes.len(), 3);
    let mut view_b: View = HashMap::new();
    apply_remote(&mut view_b, &page.changes);
    assert_eq!(view_a, view_b);

    // B edits e1 and deletes e2
    let b_changes = vec![
        with_device(change(EntityKind::Course, "e1", 2_000, false), "B"),
        with_device(change(EntityKind::Course, "e2", 2_001, true), "B"),
    ];
    let pushed_b = push_ok(&client, &server.base, &token, b_changes.clone()).await;
    assert_eq!(pushed_b.applied.len(), 2);
    apply_remote(&mut view_b, &b_changes);

    // A pulls B's changes and applies them
    let page = pull(&client, &server.base, &token, a_cursor).await;
    assert_eq!(page.changes.len(), 2);
    apply_remote(&mut view_a, &page.changes);

    // convergence: identical per-entity views
    assert_eq!(view_a, view_b);
    assert_eq!(
        view_a.get(&("course".to_string(), "e1".to_string())),
        Some(&(2_000, false))
    );
    assert_eq!(
        view_a.get(&("course".to_string(), "e2".to_string())),
        Some(&(2_001, true))
    );
    assert_eq!(
        view_a.get(&("memo".to_string(), "e3".to_string())),
        Some(&(1_002, false))
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn seq_monotonic_under_interleaved_pushes() {
    let server = start_server().await;
    let client = Client::new();
    let auth = register(&client, &server.base, "concurrent", "password123").await;
    let token = auth.token;

    let device_a = vec![
        with_device(change(EntityKind::Course, "x", 1_000, false), "A"),
        with_device(change(EntityKind::Memo, "m1", 1_000, false), "A"),
    ];
    let device_b = vec![
        with_device(change(EntityKind::Course, "x", 5_000, false), "B"),
        with_device(change(EntityKind::Memo, "m2", 1_000, false), "B"),
    ];

    let (first, second) = tokio::join!(
        push(&client, &server.base, &token, device_a),
        push(&client, &server.base, &token, device_b),
    );
    assert_eq!(first.status(), 200);
    assert_eq!(second.status(), 200);

    let page = pull(&client, &server.base, &token, 0).await;
    assert_eq!(page.changes.len(), 3);

    let mut sequences: Vec<u64> = page
        .changes
        .iter()
        .map(|change| change.server_seq.expect("server_seq present"))
        .collect();
    sequences.sort_unstable();
    let mut deduped = sequences.clone();
    deduped.dedup();
    assert_eq!(sequences, deduped, "server_seq values must be unique");
    assert!(sequences.iter().all(|seq| *seq >= 1));

    // final state of the contended entity is the LWW winner
    let winner = page
        .changes
        .iter()
        .find(|change| change.id == "x")
        .expect("entity x present");
    assert_eq!(winner.updated_at, 5_000);
}
