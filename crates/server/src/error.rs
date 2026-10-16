//! 统一错误响应：`{"error":{"code":…,"message":…}}`（协议 §8）。

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use course_plan_sync_proto::{ApiError, ApiErrorBody, ErrorCode};

#[derive(Debug, Clone)]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, message)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = StatusCode::from_u16(self.code.http_status())
            .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        let body = ApiError {
            error: ApiErrorBody {
                code: self.code,
                message: self.message,
            },
        };
        (status, Json(body)).into_response()
    }
}

impl From<sqlx::Error> for AppError {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(%error, "database error");
        AppError::internal("database error")
    }
}
