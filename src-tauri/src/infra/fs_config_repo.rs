use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};

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
        let raw = self.storage.read_to_string(&self.config_path).map_err(|_| {
            AppError::Config(format!("配置文件不存在或无法读取: {}", self.config_path))
        })?;
        serde_json::from_str::<T>(&raw).map_err(|e| {
            AppError::Serialization(format!("配置 JSON 解析失败: {}", e))
        })
    }

    fn save_config<T: Serialize>(&self, config: &T) -> AppResult<()> {
        let json = serde_json::to_string_pretty(config).map_err(|e| {
            AppError::Serialization(format!("配置序列化失败: {}", e))
        })?;
        self.storage.write_string(&self.config_path, &json)
    }

    fn config_exists(&self) -> bool {
        self.storage.exists(&self.config_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    struct TestConfig {
        name: String,
        theme: String,
        timeout: u64,
    }

    impl Default for TestConfig {
        fn default() -> Self {
            Self {
                name: "Hinina".into(),
                theme: "dark".into(),
                timeout: 30,
            }
        }
    }

    fn repo(name: &str) -> FsConfigRepository {
        let dir = std::env::temp_dir().join(format!("hinina-test-config-{}", name));
        let _ = std::fs::remove_dir_all(&dir);
        FsConfigRepository::new(Arc::new(Storage::new(dir)), "config.json")
    }

    #[test]
    fn save_and_load_roundtrip() {
        let r = repo("roundtrip");
        let cfg = TestConfig::default();

        r.save_config(&cfg).unwrap();
        assert!(r.config_exists());

        let loaded: TestConfig = r.load_config().unwrap();
        assert_eq!(loaded, cfg);
    }

    #[test]
    fn config_exists_returns_false_initially() {
        let r = repo("exists-false");
        assert!(!r.config_exists());
    }

    #[test]
    fn load_missing_config_returns_error() {
        let r = repo("missing");
        let result: AppResult<TestConfig> = r.load_config();
        assert!(result.is_err());
    }

    #[test]
    fn load_invalid_json_returns_error() {
        let r = repo("invalid-json");
        // 手动写入非法 JSON
        r.storage.write_string("config.json", "not valid json").unwrap();
        let result: AppResult<TestConfig> = r.load_config();
        assert!(result.is_err());
    }

    #[test]
    fn save_then_exists() {
        let r = repo("save-exists");
        assert!(!r.config_exists());
        r.save_config(&TestConfig::default()).unwrap();
        assert!(r.config_exists());
    }
}
