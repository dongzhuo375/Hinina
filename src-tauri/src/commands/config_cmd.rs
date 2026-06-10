use serde::{Deserialize, Serialize};
use tauri::State;

use crate::core::context::AppContext;
use crate::core::error::AppResult;

/// 应用配置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub current_oj: String,
    pub auto_save_interval_ms: u64,
    pub theme: String,
    pub language: String,
    pub editor_font_size: u32,
}

/// 获取配置
/// invoke('config:get')
#[tauri::command]
pub async fn get_config(ctx: State<'_, AppContext>) -> AppResult<AppConfig> {
    let _ = ctx;
    todo!("config_cmd::get_config()")
}

/// 更新配置
/// invoke('config:update', { config })
#[tauri::command]
pub async fn update_config(ctx: State<'_, AppContext>, config: AppConfig) -> AppResult<()> {
    let _ = (ctx, config);
    todo!("config_cmd::update_config()")
}
