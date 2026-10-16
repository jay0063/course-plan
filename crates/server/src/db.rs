//! SQLite 连接、PRAGMA 与内嵌迁移。

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::Context;
use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

/// 打开（必要时创建）SQLite 数据库并应用协议 §8 要求的 PRAGMA：
/// `journal_mode=WAL`、`busy_timeout=5000ms`、`foreign_keys=ON`。
pub async fn connect(path: &Path) -> anyhow::Result<SqlitePool> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .busy_timeout(Duration::from_millis(5_000))
        .foreign_keys(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(8)
        .acquire_timeout(Duration::from_secs(10))
        .connect_with(options)
        .await
        .with_context(|| format!("failed to open SQLite database at {}", path.display()))?;

    Ok(pool)
}

/// 运行内嵌迁移（协议 §8）；启动时自动执行。
pub async fn run_migrations(pool: &SqlitePool) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .context("failed to run database migrations")?;
    Ok(())
}

/// 当前 unix 毫秒。
pub fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or(0)
}
