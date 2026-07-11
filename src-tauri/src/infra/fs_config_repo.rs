use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};
use tracing::{debug, warn};

use crate::core::error::{AppError, AppResult};
use crate::core::repository::config_repo::ConfigRepository;
use crate::infra::storage::Storage;

/// ConfigRepository 的文件系统实现。
///
/// 配置以 JSON 格式存储在 `{storage.base_dir}/{config_path}` 单文件中。
pub struct FsConfigRepository {
    storage: Arc<Storage>,
    config_path: String,
}

impl FsConfigRepository {
    pub fn new(storage: Arc<Storage>, config_path: &str) -> Self {
        Self {
            storage,
            config_path: config_path.to_string(),
        }
    }
}

impl ConfigRepository for FsConfigRepository {
    fn load_config<T: DeserializeOwned>(&self) -> AppResult<T> {
        let raw = self.storage.read_to_string(&self.config_path).map_err(|e| {
            warn!(
                config_path = %self.config_path,
                error = %e,
                "配置文件读取失败"
            );
            AppError::Config(format!("配置文件不存在或无法读取: {}", self.config_path))
        })?;
        debug!(config_path = %self.config_path, "配置文件已加载");
        serde_json::from_str::<T>(&raw).map_err(|e| {
            warn!(
                config_path = %self.config_path,
                error = %e,
                "配置文件 JSON 解析失败"
            );
            AppError::Serialization(format!("配置 JSON 解析失败: {}", e))
        })
    }

    fn save_config<T: Serialize>(&self, config: &T) -> AppResult<()> {
        let json = serde_json::to_string_pretty(config).map_err(|e| {
            warn!(error = %e, "配置序列化失败");
            AppError::Serialization(format!("配置序列化失败: {}", e))
        })?;
        debug!(config_path = %self.config_path, "保存配置文件");
        self.storage.write_string(&self.config_path, &json).map_err(|e| {
            warn!(
                config_path = %self.config_path,
                error = %e,
                "配置文件写入失败"
            );
            e
        })
    }

    fn config_exists(&self) -> bool {
        self.storage.exists(&self.config_path)
    }
}

#[cfg(test)]
#[path = "tests/fs_config_repo_tests.rs"]
mod tests;
