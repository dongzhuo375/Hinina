//! @plugin-module
//! @layer plugin/registry
//! @version 1
//! @stability experimental
//! 未来插件发现/代码加载端口。端口属于扩展层，core 不依赖插件清单。

use crate::plugin::api::PluginResult;
use crate::plugin::host::manifest::PluginManifest;

/// @plugin-api
/// @since 1.0
/// @stability experimental
/// 仓库端口，不代表已经实现目录扫描；I/O 实现属于 infra。
pub trait PluginRepository: Send + Sync {
    /// @plugin-api
    /// @since 1.0
    /// @stability experimental
    /// 发现清单。未来实现必须隔离单插件错误；不自动执行插件或授予权限。
    fn scan_plugins(&self) -> PluginResult<Vec<PluginManifest>>;
    /// @plugin-api
    /// @since 1.0
    /// @stability experimental
    /// 校验入口存在、canonical 路径在插件根内（包括符号链接），再有限读取源码。
    fn load_plugin_code(&self, manifest: &PluginManifest) -> PluginResult<String>;
    /// @plugin-api
    /// @since 1.0
    /// @stability experimental
    /// 未实现时返回 Unsupported，不能用 false 伪装为空目录。
    fn plugin_dir_exists(&self) -> PluginResult<bool>;
}
