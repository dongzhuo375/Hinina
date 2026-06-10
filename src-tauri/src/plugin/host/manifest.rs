use serde::{Deserialize, Serialize};

/// 插件 Manifest。
///
/// 插件根目录下的 manifest.json 描述文件。
/// 定义插件的元数据、所需权限和扩展点。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    /// 唯一插件 ID
    pub id: String,

    /// 插件名称
    pub name: String,

    /// 语义化版本号
    pub version: String,

    /// 描述
    pub description: String,

    /// 作者
    pub author: String,

    /// 插件声明的权限列表
    pub permissions: Vec<PluginPermission>,

    /// 插件注册的扩展点
    pub extension_points: Vec<crate::plugin::host::extension::ExtensionPoint>,

    /// 插件最低兼容的应用版本
    pub min_app_version: String,
}

/// 插件权限
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PluginPermission {
    WorkspaceRead,
    WorkspaceWrite,
    ProblemRead,
    ContestRead,
    Notification,
}
