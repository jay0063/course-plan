//! 服务端运行时配置（协议 §5 契约）。

use std::path::PathBuf;

use anyhow::Context;

/// 冻结的二进制/环境变量契约（PLAN §5）：
/// - `PORT` 默认 `8080`
/// - `DATA_DIR` 默认 `./data`；数据库文件 `<DATA_DIR>/courseplan.db`
/// - `JWT_SECRET` 必填（缺失即启动失败）
/// - `RUST_LOG` 默认 `info`（由 tracing 初始化读取）
#[derive(Clone, Debug)]
pub struct Config {
    pub port: u16,
    pub data_dir: PathBuf,
    pub jwt_secret: String,
}

impl Config {
    /// 从环境变量读取配置；`JWT_SECRET` 缺失/为空时立即失败并给出清晰错误。
    pub fn from_env() -> anyhow::Result<Self> {
        let port = match std::env::var("PORT") {
            Ok(raw) if !raw.trim().is_empty() => raw
                .trim()
                .parse::<u16>()
                .with_context(|| format!("invalid PORT value: {raw:?} (expected 0-65535)"))?,
            _ => 8080,
        };

        let data_dir = std::env::var("DATA_DIR")
            .ok()
            .filter(|value| !value.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("./data"));

        let jwt_secret = std::env::var("JWT_SECRET")
            .ok()
            .filter(|value| !value.is_empty())
            .context("JWT_SECRET environment variable is required (set a strong random secret)")?;

        Ok(Self {
            port,
            data_dir,
            jwt_secret,
        })
    }

    /// 数据库文件路径：`<DATA_DIR>/courseplan.db`。
    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("courseplan.db")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_path_is_data_dir_courseplan_db() {
        let config = Config {
            port: 8080,
            data_dir: PathBuf::from("/data"),
            jwt_secret: "secret".into(),
        };
        assert_eq!(config.database_path(), PathBuf::from("/data/courseplan.db"));
    }
}
