//! CoursePlan 同步线协议共享类型与规则 —— 客户端与服务端的唯一真源。
//!
//! 语义真源见 `docs/SYNC-PROTOCOL.md`（v1.2）。客户端与服务端必须使用本 crate 的
//! 类型，禁止各自复制一份 DTO。

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 时钟偏移允许量（毫秒）：服务端 clamp 上限为 `now + 该值`（协议 §2.3）。
pub const CLOCK_SKEW_ALLOWANCE_MS: i64 = 300_000;
/// 单次 push 请求最多变更条数（协议 §4）。
pub const MAX_PUSH_CHANGES: usize = 500;
/// 单次 push 请求最大序列化负载：3 MiB（协议 §4）。
pub const MAX_PUSH_BYTES: usize = 3 * 1024 * 1024;
/// 单批 pull 最多返回条数。
pub const MAX_PULL_BATCH: usize = 1000;
/// 单条 payload 最大字节数：256 KiB。
pub const MAX_PAYLOAD_BYTES: usize = 256 * 1024;
/// 服务端请求体上限：4 MiB。
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;

/// 实体类型（线协议 kind）。
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    Profile,
    Course,
    Session,
    Event,
    Reminder,
    Checkin,
    Memo,
}

impl EntityKind {
    /// 全部类型，固定顺序（用于 UI 统计展示）。
    pub const ALL: [EntityKind; 7] = [
        EntityKind::Profile,
        EntityKind::Course,
        EntityKind::Session,
        EntityKind::Event,
        EntityKind::Reminder,
        EntityKind::Checkin,
        EntityKind::Memo,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            EntityKind::Profile => "profile",
            EntityKind::Course => "course",
            EntityKind::Session => "session",
            EntityKind::Event => "event",
            EntityKind::Reminder => "reminder",
            EntityKind::Checkin => "checkin",
            EntityKind::Memo => "memo",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "profile" => Some(EntityKind::Profile),
            "course" => Some(EntityKind::Course),
            "session" => Some(EntityKind::Session),
            "event" => Some(EntityKind::Event),
            "reminder" => Some(EntityKind::Reminder),
            "checkin" => Some(EntityKind::Checkin),
            "memo" => Some(EntityKind::Memo),
            _ => None,
        }
    }

    /// 中文标签（首登对话框统计等 UI 文案使用）。
    pub fn label_cn(self) -> &'static str {
        match self {
            EntityKind::Profile => "学生档案",
            EntityKind::Course => "课程",
            EntityKind::Session => "课表安排",
            EntityKind::Event => "日程",
            EntityKind::Reminder => "提醒",
            EntityKind::Checkin => "打卡任务",
            EntityKind::Memo => "备忘",
        }
    }
}

/// 一条同步变更（协议 §4）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Change {
    pub kind: EntityKind,
    pub id: String,
    /// 本地 unix 毫秒；服务端写入前 clamp（§2.3）。
    pub updated_at: i64,
    /// 墓碑标记；删除即置 true 并盖时间戳。
    pub deleted: bool,
    #[serde(default)]
    pub device_id: String,
    /// 实体 JSON；墓碑时可为 null。
    #[serde(default)]
    pub payload: Option<Value>,
    /// 仅服务端下发时存在。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_seq: Option<u64>,
}

/// LWW 谓词：严格大于才获胜；平局 → 已存在者（服务端）获胜（协议 §2.2）。
pub fn wins(incoming_updated_at: i64, existing_updated_at: i64) -> bool {
    incoming_updated_at > existing_updated_at
}

/// 服务端写入前的 clamp：不超过 `server_now + CLOCK_SKEW_ALLOWANCE_MS`（协议 §2.3）。
pub fn clamp_updated_at(updated_at_ms: i64, server_now_ms: i64) -> i64 {
    updated_at_ms.min(server_now_ms.saturating_add(CLOCK_SKEW_ALLOWANCE_MS))
}

/// 客户端出站时间戳收敛上限（协议 §2.5）：只影响出站 payload，本地保留原戳。
pub fn outbound_cap(updated_at_ms: i64, last_server_time_ms: i64) -> i64 {
    updated_at_ms.min(last_server_time_ms.saturating_add(CLOCK_SKEW_ALLOWANCE_MS))
}

// ─────────────────────────────── 认证 ───────────────────────────────

/// 注册 / 登录请求体。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct UserInfo {
    pub id: i64,
    pub username: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AuthResponse {
    pub token: String,
    pub user: UserInfo,
}

// ─────────────────────────────── 同步 ───────────────────────────────

/// `GET /api/sync/pull` 响应（协议 §4）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PullResponse {
    pub changes: Vec<Change>,
    pub latest_seq: u64,
    pub has_more: bool,
    pub server_time_ms: i64,
}

/// `POST /api/sync/push` 请求体。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushRequest {
    pub changes: Vec<Change>,
}

/// 被服务端接受的变更回执。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushAck {
    pub kind: EntityKind,
    pub id: String,
}

/// `POST /api/sync/push` 响应（协议 §4）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PushResponse {
    pub applied: Vec<PushAck>,
    /// 平局或判负时返回服务端权威版本，客户端走统一 apply（§2.4）。
    pub overridden: Vec<Change>,
    pub latest_seq: u64,
    pub server_time_ms: i64,
}

/// `GET /api/health` 响应（无鉴权）。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
}

// ─────────────────────────────── 错误 ───────────────────────────────

/// 服务端错误码（协议 §8）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidCredentials,
    UsernameTaken,
    InvalidRequest,
    Unauthorized,
    PayloadTooLarge,
    Internal,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            ErrorCode::InvalidCredentials => "invalid_credentials",
            ErrorCode::UsernameTaken => "username_taken",
            ErrorCode::InvalidRequest => "invalid_request",
            ErrorCode::Unauthorized => "unauthorized",
            ErrorCode::PayloadTooLarge => "payload_too_large",
            ErrorCode::Internal => "internal",
        }
    }

    pub fn http_status(self) -> u16 {
        match self {
            ErrorCode::InvalidCredentials => 401,
            ErrorCode::UsernameTaken => 409,
            ErrorCode::InvalidRequest => 400,
            ErrorCode::Unauthorized => 401,
            ErrorCode::PayloadTooLarge => 413,
            ErrorCode::Internal => 500,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiErrorBody {
    pub code: ErrorCode,
    pub message: String,
}

/// 统一错误响应：`{"error": {"code": "...", "message": "..."}}`。
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApiError {
    pub error: ApiErrorBody,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wins_is_strict_and_favours_existing_on_tie() {
        assert!(wins(5, 4));
        assert!(!wins(4, 4));
        assert!(!wins(4, 5));
        assert!(!wins(0, 0));
        assert!(wins(i64::MAX, i64::MAX - 1));
    }

    #[test]
    fn clamp_caps_future_stamps_only() {
        let now = 1_750_000_000_000;
        assert_eq!(
            clamp_updated_at(now + CLOCK_SKEW_ALLOWANCE_MS * 10, now),
            now + CLOCK_SKEW_ALLOWANCE_MS
        );
        assert_eq!(clamp_updated_at(now - 1, now), now - 1);
        assert_eq!(clamp_updated_at(now, now), now);
        // 极端：i64::MAX 不溢出
        assert_eq!(
            clamp_updated_at(i64::MAX, i64::MAX),
            i64::MAX // saturating_add 后为 MAX，min 取原值
        );
    }

    #[test]
    fn outbound_cap_limits_far_future() {
        let server = 1_750_000_000_000;
        assert_eq!(
            outbound_cap(server + 10 * CLOCK_SKEW_ALLOWANCE_MS, server),
            server + CLOCK_SKEW_ALLOWANCE_MS
        );
        assert_eq!(outbound_cap(server - 5, server), server - 5);
    }

    #[test]
    fn entity_kind_roundtrip() {
        for kind in EntityKind::ALL {
            assert_eq!(EntityKind::parse(kind.as_str()), Some(kind));
            assert!(!kind.label_cn().is_empty());
        }
        assert_eq!(EntityKind::parse("bogus"), None);
        assert_eq!(EntityKind::ALL.len(), 7);
    }

    #[test]
    fn change_minimal_json_parses_with_defaults() {
        let text = r#"{"kind":"course","id":"abc","updated_at":1,"deleted":false}"#;
        let change: Change = serde_json::from_str(text).unwrap();
        assert_eq!(change.kind, EntityKind::Course);
        assert_eq!(change.device_id, "");
        assert!(change.payload.is_none());
        assert!(change.server_seq.is_none());
    }

    #[test]
    fn change_full_roundtrip() {
        let change = Change {
            kind: EntityKind::Checkin,
            id: "c1".into(),
            updated_at: 1_750_000_000_123,
            deleted: false,
            device_id: "dev-x1".into(),
            payload: Some(serde_json::json!({"title": "阅读", "history": {"2026-10-17": true}})),
            server_seq: Some(42),
        };
        let text = serde_json::to_string(&change).unwrap();
        let back: Change = serde_json::from_str(&text).unwrap();
        assert_eq!(back.kind, EntityKind::Checkin);
        assert_eq!(back.server_seq, Some(42));
        assert!(back.payload.is_some());
    }

    #[test]
    fn tombstones_serialize_payload_as_null_or_absent() {
        let text = r#"{"kind":"memo","id":"m","updated_at":9,"deleted":true,"device_id":"d"}"#;
        let change: Change = serde_json::from_str(text).unwrap();
        assert!(change.deleted);
        assert!(change.payload.is_none());
        let out = serde_json::to_string(&change).unwrap();
        assert!(!out.contains("server_seq")); // skip_serializing_if
    }

    #[test]
    fn pull_and_push_response_wire_shapes() {
        let pull =
            r#"{"changes":[],"latest_seq":184,"has_more":false,"server_time_ms":1750000123456}"#;
        let parsed: PullResponse = serde_json::from_str(pull).unwrap();
        assert_eq!(parsed.latest_seq, 184);
        assert!(!parsed.has_more);

        let push = r#"{"applied":[{"kind":"course","id":"abc"}],"overridden":[],"latest_seq":186,"server_time_ms":1750000123456}"#;
        let parsed: PushResponse = serde_json::from_str(push).unwrap();
        assert_eq!(parsed.applied.len(), 1);
        assert_eq!(parsed.applied[0].kind, EntityKind::Course);
    }

    #[test]
    fn error_code_shapes() {
        assert_eq!(
            serde_json::to_string(&ErrorCode::InvalidCredentials).unwrap(),
            "\"invalid_credentials\""
        );
        assert_eq!(ErrorCode::UsernameTaken.http_status(), 409);
        assert_eq!(ErrorCode::PayloadTooLarge.http_status(), 413);
        let err: ApiError =
            serde_json::from_str(r#"{"error":{"code":"unauthorized","message":"token expired"}}"#)
                .unwrap();
        assert_eq!(err.error.code, ErrorCode::Unauthorized);
    }

    #[test]
    fn credentials_shape() {
        let c: Credentials =
            serde_json::from_str(r#"{"username":"xiaoman","password":"12345678"}"#).unwrap();
        assert_eq!(c.username, "xiaoman");
    }

    #[test]
    #[allow(clippy::assertions_on_constants)] // 有意对冻结常量断言：接口冻结检查
    fn limits_match_protocol_doc() {
        assert_eq!(MAX_PUSH_CHANGES, 500);
        assert_eq!(MAX_PULL_BATCH, 1000);
        assert_eq!(MAX_PUSH_BYTES, 3 * 1024 * 1024);
        assert_eq!(MAX_PAYLOAD_BYTES, 256 * 1024);
        assert_eq!(MAX_BODY_BYTES, 4 * 1024 * 1024);
        assert!(MAX_PUSH_BYTES < MAX_BODY_BYTES);
    }
}
