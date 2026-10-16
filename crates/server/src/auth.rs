//! 认证：注册 / 登录 / JWT 鉴权提取器（协议 §3）。

use std::sync::OnceLock;

use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::Json;
use axum::extract::{FromRequestParts, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use course_plan_sync_proto::{AuthResponse, Credentials, ErrorCode, UserInfo};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use sqlx::Row;

use crate::db::now_ms;
use crate::error::AppError;
use crate::json::AppJson;
use crate::state::AppState;

/// 会话有效期：30 天（协议 §3）。
const TOKEN_TTL_SECS: i64 = 30 * 24 * 60 * 60;

#[derive(Debug, Serialize, Deserialize)]
struct Claims {
    /// 用户 id。
    sub: i64,
    /// 过期时间（unix 秒）。
    exp: i64,
}

/// 已鉴权用户，作为 axum 提取器注入受保护路由。
pub struct AuthUser {
    pub id: i64,
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let header = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| {
                AppError::new(ErrorCode::Unauthorized, "missing Authorization header")
            })?;

        let token = header
            .strip_prefix("Bearer ")
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .ok_or_else(|| {
                AppError::new(
                    ErrorCode::Unauthorized,
                    "invalid Authorization header: expected `Bearer <token>`",
                )
            })?;

        let mut validation = Validation::new(Algorithm::HS256);
        validation.validate_exp = true;

        let decoded = decode::<Claims>(
            token,
            &DecodingKey::from_secret(state.jwt_secret.as_bytes()),
            &validation,
        )
        .map_err(|_| AppError::new(ErrorCode::Unauthorized, "invalid or expired token"))?;

        Ok(AuthUser {
            id: decoded.claims.sub,
        })
    }
}

/// `POST /api/auth/register`
pub async fn register(
    State(state): State<AppState>,
    AppJson(credentials): AppJson<Credentials>,
) -> Result<Json<AuthResponse>, AppError> {
    validate_username(&credentials.username)?;
    if credentials.password.chars().count() < 8 {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "password must be at least 8 characters",
        ));
    }

    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(credentials.password.as_bytes(), &salt)
        .map_err(|error| {
            tracing::error!(%error, "password hashing failed");
            AppError::internal("failed to hash password")
        })?
        .to_string();

    let created_at = now_ms();
    let insert =
        sqlx::query("INSERT INTO users (username, password_hash, created_at) VALUES (?, ?, ?)")
            .bind(&credentials.username)
            .bind(&password_hash)
            .bind(created_at)
            .execute(&state.pool)
            .await;

    let user_id = match insert {
        Ok(result) => result.last_insert_rowid(),
        Err(sqlx::Error::Database(database_error)) if database_error.is_unique_violation() => {
            return Err(AppError::new(
                ErrorCode::UsernameTaken,
                "username already taken",
            ));
        }
        Err(error) => return Err(error.into()),
    };

    let token = issue_token(&state, user_id)?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user_id,
            username: credentials.username,
        },
    }))
}

/// `POST /api/auth/login`
pub async fn login(
    State(state): State<AppState>,
    AppJson(credentials): AppJson<Credentials>,
) -> Result<Json<AuthResponse>, AppError> {
    let invalid = || {
        AppError::new(
            ErrorCode::InvalidCredentials,
            "invalid username or password",
        )
    };

    let row = sqlx::query(
        "SELECT id, username, password_hash FROM users WHERE username = ? COLLATE NOCASE",
    )
    .bind(&credentials.username)
    .fetch_optional(&state.pool)
    .await?;

    let Some(row) = row else {
        // 未知用户：仍做一次等价的哈希校验，降低用户名枚举的时间侧信道。
        verify_password_dummy(&credentials.password);
        return Err(invalid());
    };

    let user_id: i64 = row.try_get("id")?;
    let username: String = row.try_get("username")?;
    let password_hash: String = row.try_get("password_hash")?;

    let parsed_hash = PasswordHash::new(&password_hash).map_err(|error| {
        tracing::error!(%error, "stored password hash is invalid");
        AppError::internal("stored password hash is invalid")
    })?;

    if Argon2::default()
        .verify_password(credentials.password.as_bytes(), &parsed_hash)
        .is_err()
    {
        return Err(invalid());
    }

    let token = issue_token(&state, user_id)?;
    Ok(Json(AuthResponse {
        token,
        user: UserInfo {
            id: user_id,
            username,
        },
    }))
}

fn validate_username(username: &str) -> Result<(), AppError> {
    let length = username.chars().count();
    if !(2..=32).contains(&length) {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "username must be 2-32 characters",
        ));
    }
    if username
        .chars()
        .any(|character| character.is_whitespace() || character.is_control())
    {
        return Err(AppError::new(
            ErrorCode::InvalidRequest,
            "username must not contain whitespace or control characters",
        ));
    }
    Ok(())
}

fn issue_token(state: &AppState, user_id: i64) -> Result<String, AppError> {
    let exp = now_ms() / 1000 + TOKEN_TTL_SECS;
    let claims = Claims { sub: user_id, exp };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(state.jwt_secret.as_bytes()),
    )
    .map_err(|error| {
        tracing::error!(%error, "failed to encode JWT");
        AppError::internal("failed to issue token")
    })
}

static DUMMY_HASH: OnceLock<String> = OnceLock::new();

fn verify_password_dummy(password: &str) {
    let hash = DUMMY_HASH.get_or_init(|| {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(b"course-plan-dummy-password", &salt)
            .map(|hash| hash.to_string())
            .unwrap_or_default()
    });
    if let Ok(parsed) = PasswordHash::new(hash) {
        let _ = Argon2::default().verify_password(password.as_bytes(), &parsed);
    }
}
