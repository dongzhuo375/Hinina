use serde::Serialize;
use tauri::State;
use tracing::info;

use crate::core::context::AppContext;
use crate::core::entity::config::AppConfig;
use crate::core::error::AppResult;

/// 存储与版本信息（排障用：前端「关于/诊断」面板展示日志位置与客户端版本）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    /// 应用数据根目录
    pub base_dir: String,
    /// 日志文件完整路径（与 `infra/logger.rs` 的落盘位置一致）
    pub log_path: String,
    /// 客户端版本（构建时注入的 Cargo 包版本，与 HttpClient UA 同源）
    pub version: String,
}

/// 获取存储与版本信息。
///
/// 前端 invoke 签名: `get_storage_info`
#[tauri::command]
pub async fn get_storage_info(ctx: State<'_, AppContext>) -> AppResult<StorageInfo> {
    let base_dir = ctx.storage.base_dir();
    Ok(StorageInfo {
        log_path: base_dir
            .join("logs")
            .join("hinina.log")
            .to_string_lossy()
            .into_owned(),
        base_dir: base_dir.to_string_lossy().into_owned(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

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
