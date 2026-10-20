//! @plugin-module
//! @layer infra
//! @version 1
//! @stability experimental
//! ????????????? Unsupported?????????? panic?

use crate::plugin::api::{PluginError, PluginResult};
use crate::plugin::host::manifest::PluginManifest;
use crate::plugin::registry::PluginRepository;

/// @plugin-api
/// @since 1.0
/// @stability experimental
/// ????????????? AppContext???? Storage ??????
#[derive(Default)]
pub struct FsPluginRepository;

impl FsPluginRepository {
    /// @plugin-api
    /// @since 1.0
    /// @stability experimental
    /// ????????? I/O?
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl PluginRepository for FsPluginRepository {
    fn scan_plugins(&self) -> PluginResult<Vec<PluginManifest>> {
        Err(PluginError::Unsupported)
    }
    fn load_plugin_code(&self, _manifest: &PluginManifest) -> PluginResult<String> {
        Err(PluginError::Unsupported)
    }
    fn plugin_dir_exists(&self) -> PluginResult<bool> {
        Err(PluginError::Unsupported)
    }
}

#[cfg(test)]
#[path = "tests/fs_plugin_repo_tests.rs"]
mod tests;
