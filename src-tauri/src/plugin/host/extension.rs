//! @plugin-module
//! @layer plugin/host
//! @version 1
//! @stability experimental
//!
use serde::{Deserialize, Serialize};

/// 扩展点枚举。
///
/// 插件通过注册扩展点来增强客户端 UI。
/// 仅保留远期类型草案，当前清单不接受非空 contributes。
/// @plugin-api
/// @since 1.0
/// @stability experimental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtensionPoint {
    Sidebar(SidebarExtension),
    Command(CommandExtension),
    StatusBar(StatusBarExtension),
    Editor(EditorExtension),
}

/// 侧边栏扩展点
/// @plugin-api
/// @since 1.0
/// @stability experimental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarExtension {
    pub id: String,
    pub title: String,
    pub icon: String,
}

/// 命令面板扩展点
/// @plugin-api
/// @since 1.0
/// @stability experimental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExtension {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
}

/// 状态栏扩展点
/// @plugin-api
/// @since 1.0
/// @stability experimental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBarExtension {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
}

/// 编辑器扩展点（Monaco Editor 增强）
/// @plugin-api
/// @since 1.0
/// @stability experimental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorExtension {
    pub id: String,
    pub extension_type: EditorExtensionType,
}

/// @plugin-api
/// @since 1.0
/// @stability experimental
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorExtensionType {
    Completion,
    Hover,
    Decoration,
}
