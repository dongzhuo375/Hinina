use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::config::ThemeConfig;
use crate::core::error::AppResult;

/// 获取当前主题配置。
///
/// 前端 invoke 签名: `get_theme`
///
/// 返回 `ThemeConfig`（主题名称 + 编辑器主题），从 ConfigService 读取。
#[tauri::command]
pub async fn get_theme(ctx: State<'_, AppContext>) -> AppResult<ThemeConfig> {
    Ok(ctx.theme.get_theme_config())
}

/// 切换主题。
///
/// 前端 invoke 签名: `set_theme`({ themeName })
///
/// 支持 "light" / "dark"，自动匹配 Monaco Editor 主题（vs / vs-dark）。
/// 发布 `CoreEvent::ThemeChanged` 通知其他观察者（前端主题由 IPC 返回值直接应用）。
#[tauri::command]
pub async fn set_theme(ctx: State<'_, AppContext>, theme_name: String) -> AppResult<()> {
    info!(theme = %theme_name, "Command: 切换主题");
    ctx.theme.set_theme(&theme_name)
}
