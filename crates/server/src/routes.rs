//! 路由装配。

use axum::extract::DefaultBodyLimit;
use axum::routing::{get, post};
use axum::{Json, Router};
use course_plan_sync_proto::{HealthResponse, MAX_BODY_BYTES};

use crate::state::AppState;
use crate::{auth, sync};

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/api/auth/register", post(auth::register))
        .route("/api/auth/login", post(auth::login))
        .route("/api/sync/pull", get(sync::pull))
        .route("/api/sync/push", post(sync::push))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}
