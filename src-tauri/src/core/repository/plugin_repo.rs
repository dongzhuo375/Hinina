use crate::core::error::AppResult;
use crate::plugin::host::manifest::PluginManifest;

/// 插件仓库。
///
/// 抽象插件目录扫描与代码加载，解耦 PluginHost 与底层文件系统。
pub trait PluginRepository: Send + Sync {
    /// 扫描插件目录，返回所有找到的 PluginManifest
    fn scan_plugins(&self) -> AppResult<Vec<PluginManifest>>;

    /// 加载指定插件的入口代码
    fn load_plugin_code(&self, plugin_id: &str) -> AppResult<String>;

    /// 检查插件目录是否存在
    fn plugin_dir_exists(&self) -> bool;
}
