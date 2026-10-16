//! 同步接口：`GET /api/sync/pull` 与 `POST /api/sync/push`（协议 §4）。

use axum::Json;
use axum::extract::{Query, State};
use course_plan_sync_proto::{
    Change, EntityKind, ErrorCode, MAX_PAYLOAD_BYTES, MAX_PULL_BATCH, MAX_PUSH_BYTES,
    MAX_PUSH_CHANGES, PullResponse, PushAck, PushRequest, PushResponse, clamp_updated_at, wins,
};
use serde::Deserialize;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::auth::AuthUser;
use crate::db::now_ms;
use crate::error::AppError;
use crate::json::AppJson;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct PullQuery {
    #[serde(default)]
    pub since: u64,
}

/// `GET /api/sync/pull?since=<u64>`
///
/// 批次与 `latest_seq` 在同一读事务内查询（协议 §4 读快照保证）。
pub async fn pull(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<PullQuery>,
) -> Result<Json<PullResponse>, AppError> {
    let mut tx = state.pool.begin().await?;

    let rows = sqlx::query(
        "SELECT kind, entity_id, payload, updated_at, device_id, deleted, server_seq \
         FROM entities WHERE account_id = ? AND server_seq > ? \
         ORDER BY server_seq ASC LIMIT ?",
    )
    .bind(user.id)
    .bind(query.since as i64)
    .bind(MAX_PULL_BATCH as i64 + 1)
    .fetch_all(&mut *tx)
    .await?;

    let has_more = rows.len() > MAX_PULL_BATCH;
    let mut changes = Vec::with_capacity(rows.len().min(MAX_PULL_BATCH));
    for row in rows.iter().take(MAX_PULL_BATCH) {
        changes.push(row_to_change(row)?);
    }

    let latest_seq: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(server_seq), 0) FROM entities WHERE account_id = ?",
    )
    .bind(user.id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(Json(PullResponse {
        changes,
        latest_seq: latest_seq.max(0) as u64,
        has_more,
        server_time_ms: now_ms(),
    }))
}

/// `POST /api/sync/push`
///
/// 单事务内完成裁决 + `seq_counter` 递增 + 行写入（协议 §4/§8）。
pub async fn push(
    State(state): State<AppState>,
    user: AuthUser,
    AppJson(body): AppJson<PushRequest>,
) -> Result<Json<PushResponse>, AppError> {
    if body.changes.len() > MAX_PUSH_CHANGES {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            format!("too many changes: at most {MAX_PUSH_CHANGES} per request"),
        ));
    }

    let serialized = serde_json::to_vec(&body.changes).map_err(|error| {
        AppError::new(
            ErrorCode::InvalidRequest,
            format!("changes could not be serialized: {error}"),
        )
    })?;
    if serialized.len() > MAX_PUSH_BYTES {
        return Err(AppError::new(
            ErrorCode::PayloadTooLarge,
            format!("push batch exceeds {MAX_PUSH_BYTES} bytes"),
        ));
    }

    for change in &body.changes {
        if let Some(payload) = &change.payload {
            let size = serde_json::to_vec(payload)
                .map_err(|error| {
                    AppError::new(
                        ErrorCode::InvalidRequest,
                        format!("payload could not be serialized: {error}"),
                    )
                })?
                .len();
            if size > MAX_PAYLOAD_BYTES {
                return Err(AppError::new(
                    ErrorCode::PayloadTooLarge,
                    format!("payload exceeds {MAX_PAYLOAD_BYTES} bytes"),
                ));
            }
        }
    }

    let now = now_ms();

    // 单写者契约：用 BEGIN IMMEDIATE 直接取得写锁，避免 SQLite 延迟事务升级死锁。
    let mut conn = state.pool.acquire().await?;
    sqlx::query("BEGIN IMMEDIATE").execute(&mut *conn).await?;

    match apply_push(&mut conn, user.id, &body.changes, now).await {
        Ok(response) => {
            if let Err(error) = sqlx::query("COMMIT").execute(&mut *conn).await {
                let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
                return Err(error.into());
            }
            Ok(Json(response))
        }
        Err(error) => {
            let _ = sqlx::query("ROLLBACK").execute(&mut *conn).await;
            Err(error)
        }
    }
}

async fn apply_push(
    conn: &mut SqliteConnection,
    account_id: i64,
    changes: &[Change],
    now: i64,
) -> Result<PushResponse, AppError> {
    let mut applied: Vec<(&Change, i64)> = Vec::new();
    let mut overridden: Vec<Change> = Vec::new();

    for change in changes {
        let incoming_updated_at = clamp_updated_at(change.updated_at, now);
        let existing = sqlx::query(
            "SELECT payload, updated_at, device_id, deleted, server_seq \
             FROM entities WHERE account_id = ? AND kind = ? AND entity_id = ?",
        )
        .bind(account_id)
        .bind(change.kind.as_str())
        .bind(&change.id)
        .fetch_optional(&mut *conn)
        .await?;

        match existing {
            Some(row) => {
                let existing_updated_at: i64 = row.try_get("updated_at")?;
                if wins(incoming_updated_at, existing_updated_at) {
                    applied.push((change, incoming_updated_at));
                } else {
                    overridden.push(existing_row_to_change(&row, change.kind, &change.id)?);
                }
            }
            None => applied.push((change, incoming_updated_at)),
        }
    }

    let applied_count = applied.len() as i64;

    sqlx::query("UPDATE users SET seq_counter = seq_counter + ? WHERE id = ?")
        .bind(applied_count)
        .bind(account_id)
        .execute(&mut *conn)
        .await?;

    let seq_counter: i64 = sqlx::query_scalar("SELECT seq_counter FROM users WHERE id = ?")
        .bind(account_id)
        .fetch_one(&mut *conn)
        .await?;

    let first_seq = seq_counter - applied_count + 1;

    let mut applied_acks = Vec::with_capacity(applied.len());
    for (index, (change, updated_at)) in applied.iter().enumerate() {
        let server_seq = first_seq + index as i64;
        let payload_text = match &change.payload {
            Some(value) => Some(serde_json::to_string(value).map_err(|error| {
                AppError::internal(format!("payload serialization failed: {error}"))
            })?),
            None => None,
        };

        sqlx::query(
            "INSERT INTO entities \
                 (account_id, kind, entity_id, payload, updated_at, device_id, deleted, server_seq) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(account_id, kind, entity_id) DO UPDATE SET \
                 payload = excluded.payload, updated_at = excluded.updated_at, \
                 device_id = excluded.device_id, deleted = excluded.deleted, \
                 server_seq = excluded.server_seq",
        )
        .bind(account_id)
        .bind(change.kind.as_str())
        .bind(&change.id)
        .bind(payload_text)
        .bind(*updated_at)
        .bind(&change.device_id)
        .bind(if change.deleted { 1_i64 } else { 0_i64 })
        .bind(server_seq)
        .execute(&mut *conn)
        .await?;

        applied_acks.push(PushAck {
            kind: change.kind,
            id: change.id.clone(),
        });
    }

    Ok(PushResponse {
        applied: applied_acks,
        overridden,
        latest_seq: seq_counter.max(0) as u64,
        server_time_ms: now,
    })
}

fn row_to_change(row: &SqliteRow) -> Result<Change, AppError> {
    let kind_text: String = row.try_get("kind")?;
    let kind = EntityKind::parse(&kind_text).ok_or_else(|| {
        AppError::internal(format!("unknown entity kind in storage: {kind_text}"))
    })?;

    Ok(Change {
        kind,
        id: row.try_get("entity_id")?,
        updated_at: row.try_get("updated_at")?,
        deleted: row.try_get::<i64, _>("deleted")? != 0,
        device_id: row.try_get("device_id")?,
        payload: decode_payload(row.try_get::<Option<String>, _>("payload")?)?,
        server_seq: Some(row.try_get::<i64, _>("server_seq")?.max(0) as u64),
    })
}

fn existing_row_to_change(row: &SqliteRow, kind: EntityKind, id: &str) -> Result<Change, AppError> {
    Ok(Change {
        kind,
        id: id.to_string(),
        updated_at: row.try_get("updated_at")?,
        deleted: row.try_get::<i64, _>("deleted")? != 0,
        device_id: row.try_get("device_id")?,
        payload: decode_payload(row.try_get::<Option<String>, _>("payload")?)?,
        server_seq: Some(row.try_get::<i64, _>("server_seq")?.max(0) as u64),
    })
}

fn decode_payload(raw: Option<String>) -> Result<Option<serde_json::Value>, AppError> {
    match raw {
        Some(text) => serde_json::from_str(&text).map(Some).map_err(|error| {
            AppError::internal(format!("stored payload is not valid JSON: {error}"))
        }),
        None => Ok(None),
    }
}
