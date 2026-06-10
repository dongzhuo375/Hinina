use std::sync::Arc;

use serde::{de::DeserializeOwned, Serialize};

use crate::core::error::AppResult;
use crate::core::repository::config_repo::ConfigRepository;
use crate::infra::storage::Storage;

/// ConfigRepository 的文件系统实现
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
        todo!("FsConfigRepository::load_config()")
    }

    fn save_config<T: Serialize>(&self, config: &T) -> AppResult<()> {
        let _ = config;
        todo!("FsConfigRepository::save_config()")
    }

    fn config_exists(&self) -> bool {
        todo!("FsConfigRepository::config_exists()")
    }
}
