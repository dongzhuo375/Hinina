use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::config::ThemeConfig;
use crate::core::error::AppResult;

/// 获取当前主题配置。
///
/// 前端 invoke 签名: `theme:get`
///
/// 返回 `ThemeConfig`（主题名称 + 编辑器主题），从 ConfigService 读取。
#[tauri::command]
pub async fn get_theme(ctx: State<'_, AppContext>) -> AppResult<ThemeConfig> {
    Ok(ctx.theme.get_theme_config())
}

/// 切换主题。
///
/// 前端 invoke 签名: `theme:set`({ theme_name })
///
/// 支持 "light" / "dark"，自动匹配 Monaco Editor 主题（vs / vs-dark）。
/// 发布 `SystemEvent::ThemeChanged` 通知前端所有组件更新样式。
#[tauri::command]
pub async fn set_theme(
    ctx: State<'_, AppContext>,
    theme_name: String,
) -> AppResult<()> {
    info!(theme = %theme_name, "Command: 切换主题");
    ctx.theme.set_theme(&theme_name)
}
