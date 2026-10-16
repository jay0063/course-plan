//! 自定义 JSON 提取器：把 axum 的 JSON / body-limit 拒绝映射为协议错误码。

use axum::Json;
use axum::extract::rejection::JsonRejection;
use axum::extract::{FromRequest, Request};
use axum::http::StatusCode;
use course_plan_sync_proto::ErrorCode;

use crate::error::AppError;

/// 与 `axum::Json` 同形，但拒绝时返回统一 `ApiError`：
/// - body 超过 `DefaultBodyLimit`（4 MiB）→ `413 payload_too_large`
/// - 其余解析/内容类型错误 → `400 invalid_request`
pub struct AppJson<T>(pub T);

impl<S, T> FromRequest<S> for AppJson<T>
where
    Json<T>: FromRequest<S, Rejection = JsonRejection>,
    S: Send + Sync,
{
    type Rejection = AppError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        match Json::<T>::from_request(request, state).await {
            Ok(Json(value)) => Ok(AppJson(value)),
            Err(rejection) => Err(map_rejection(rejection)),
        }
    }
}

fn map_rejection(rejection: JsonRejection) -> AppError {
    if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE {
        AppError::new(ErrorCode::PayloadTooLarge, rejection.body_text())
    } else {
        AppError::new(ErrorCode::InvalidRequest, rejection.body_text())
    }
}
