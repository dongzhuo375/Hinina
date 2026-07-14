use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::config::AppConfig;
use crate::core::error::AppResult;

/// 获取完整应用配置。
///
/// 前端 invoke 签名: `get_config`
///
/// 返回 `AppConfig`（包含 user/oj/editor/theme/layout 五个子分组）。
/// 配置源自磁盘持久化文件，首次启动时自动生成默认值。
#[tauri::command]
pub async fn get_config(ctx: State<'_, AppContext>) -> AppResult<AppConfig> {
    Ok(ctx.config.get())
}

/// 从磁盘重新加载配置。
///
/// 前端 invoke 签名: `reload_config`
///
/// 发布 `SystemEvent::ConfigReloaded`，各 Service 可通过监听此事件热更新参数。
#[tauri::command]
pub async fn reload_config(ctx: State<'_, AppContext>) -> AppResult<AppConfig> {
    info!("Command: 重新加载配置");
    ctx.config.reload()
}

/// 更新配置并持久化。
///
/// 前端 invoke 签名: `update_config`({ config })
///
/// 将前端传来的完整 `AppConfig` 写入磁盘。
/// 配置字段按需热生效（如主题切换需额外调用 `theme:set` 发布事件）。
#[tauri::command]
pub async fn update_config(
    ctx: State<'_, AppContext>,
    config: AppConfig,
) -> AppResult<()> {
    info!("Command: 更新配置");
    ctx.config.update(|cfg| {
        *cfg = config;
    })?;
    Ok(())
}
