//! CoursePlan 同步服务端库入口。
//!
//! 冻结契约见 `docs/PLAN.md` §5 与 `docs/SYNC-PROTOCOL.md` v1.2：
//! 二进制名 `course-plan-server`；`PORT`（默认 8080）；`DATA_DIR`（默认 `./data`，
//! DB=`<DATA_DIR>/courseplan.db`）；`JWT_SECRET` 必填；`RUST_LOG` 默认 `info`。

pub mod auth;
pub mod config;
pub mod db;
pub mod error;
pub mod json;
pub mod routes;
pub mod state;
pub mod sync;

pub use config::Config;
pub use error::AppError;
pub use state::AppState;

use std::net::SocketAddr;

use anyhow::Context;
use tokio::net::TcpListener;

/// 初始化 tracing（`RUST_LOG`，默认 `info`）。幂等（重复调用忽略错误）。
pub fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}

/// 建立数据目录、打开数据库并运行迁移。
pub async fn init_state(config: &Config) -> anyhow::Result<AppState> {
    std::fs::create_dir_all(&config.data_dir)
        .with_context(|| format!("failed to create DATA_DIR {}", config.data_dir.display()))?;

    let pool = db::connect(&config.database_path()).await?;
    db::run_migrations(&pool).await?;

    Ok(AppState::new(pool, config.jwt_secret.clone()))
}

/// 构建完整路由（含状态），供集成测试与自托管复用。
pub async fn build_app(config: &Config) -> anyhow::Result<axum::Router> {
    let state = init_state(config).await?;
    Ok(routes::router(state))
}

/// 在 `bind` 地址启动服务端，返回实际监听地址与后台任务句柄。
pub async fn spawn_server(
    config: &Config,
    bind: SocketAddr,
) -> anyhow::Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let state = init_state(config).await?;
    let app = routes::router(state);

    let listener = TcpListener::bind(bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;
    let local_addr = listener.local_addr()?;

    let handle = tokio::spawn(async move {
        if let Err(error) = axum::serve(listener, app).await {
            tracing::error!(%error, "server stopped with error");
        }
    });

    Ok((local_addr, handle))
}

/// 生产入口：读取环境变量、初始化 tracing、监听 `0.0.0.0:PORT`。
pub async fn run() -> anyhow::Result<()> {
    let config = Config::from_env()?;
    init_tracing();

    tracing::info!(
        port = config.port,
        data_dir = %config.data_dir.display(),
        "starting course-plan-server"
    );

    let bind = SocketAddr::from(([0, 0, 0, 0], config.port));
    let (addr, handle) = spawn_server(&config, bind).await?;
    tracing::info!("course-plan-server listening on http://{addr}");

    handle.await?;
    Ok(())
}
