use serde::{Deserialize, Serialize};

/// 扩展点枚举。
///
/// 插件通过注册扩展点来增强客户端 UI。
/// v0.x 仅定义接口，v1.0+ 实现渲染。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtensionPoint {
    Sidebar(SidebarExtension),
    Command(CommandExtension),
    StatusBar(StatusBarExtension),
    Editor(EditorExtension),
}

/// 侧边栏扩展点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidebarExtension {
    pub id: String,
    pub title: String,
    pub icon: String,
}

/// 命令面板扩展点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandExtension {
    pub id: String,
    pub label: String,
    pub shortcut: Option<String>,
}

/// 状态栏扩展点
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusBarExtension {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
}

/// 编辑器扩展点（Monaco Editor 增强）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditorExtension {
    pub id: String,
    pub extension_type: EditorExtensionType,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EditorExtensionType {
    Completion,
    Hover,
    Decoration,
}
