//! 共享应用状态。

use std::sync::Arc;

use sqlx::SqlitePool;

#[derive(Clone)]
pub struct AppState {
    pub pool: SqlitePool,
    pub jwt_secret: Arc<String>,
}

impl AppState {
    pub fn new(pool: SqlitePool, jwt_secret: String) -> Self {
        Self {
            pool,
            jwt_secret: Arc::new(jwt_secret),
        }
    }
}
