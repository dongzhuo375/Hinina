use serde::{Deserialize, Serialize};
use tauri::State;

use crate::core::context::AppContext;
use crate::core::error::AppResult;

/// 主题配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub mode: String,
    pub primary_color: String,
    pub font_family: String,
    pub border_radius: u32,
}

/// 获取主题
/// invoke('theme:get')
#[tauri::command]
pub async fn get_theme(ctx: State<'_, AppContext>) -> AppResult<ThemeConfig> {
    let _ = ctx;
    todo!("theme_cmd::get_theme()")
}

/// 设置主题
/// invoke('theme:set', { config })
#[tauri::command]
pub async fn set_theme(ctx: State<'_, AppContext>, config: ThemeConfig) -> AppResult<()> {
    let _ = (ctx, config);
    todo!("theme_cmd::set_theme()")
}
