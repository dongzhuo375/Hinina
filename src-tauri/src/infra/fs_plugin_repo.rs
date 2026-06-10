use std::sync::Arc;

use crate::core::error::AppResult;
use crate::core::repository::plugin_repo::PluginRepository;
use crate::infra::storage::Storage;
use crate::plugin::host::manifest::PluginManifest;

/// PluginRepository 的文件系统实现
pub struct FsPluginRepository {
    storage: Arc<Storage>,
}

impl FsPluginRepository {
    pub fn new(storage: Arc<Storage>) -> Self {
        Self { storage }
    }
}

impl PluginRepository for FsPluginRepository {
    fn scan_plugins(&self) -> AppResult<Vec<PluginManifest>> {
        todo!("FsPluginRepository::scan_plugins()")
    }

    fn load_plugin_code(&self, plugin_id: &str) -> AppResult<String> {
        let _ = plugin_id;
        todo!("FsPluginRepository::load_plugin_code()")
    }

    fn plugin_dir_exists(&self) -> bool {
        todo!("FsPluginRepository::plugin_dir_exists()")
    }
}
