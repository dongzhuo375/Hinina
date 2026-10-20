//! Developer plugin controls. These commands never accept permissions, source, URLs or file paths.
use crate::{
    core::{
        context::AppContext,
        error::{AppError, AppResult},
    },
    plugin::{api::PluginError, manager::PluginStatus},
};
use tauri::State;
fn safe(error: PluginError) -> AppError {
    AppError::Config(format!("Plugin operation failed: {}", error))
}

/// List configured plugin status using stable, redacted diagnostics.
#[tauri::command]
pub async fn plugin_list(ctx: State<'_, AppContext>) -> AppResult<Vec<PluginStatus>> {
    let manager = ctx.plugin_manager.clone();
    tokio::task::spawn_blocking(move || manager.list())
        .await
        .map_err(|_| AppError::Config("Plugin status unavailable".into()))
}
/// Start only a plugin explicitly enabled and approved in the host configuration.
#[tauri::command]
pub async fn plugin_start(ctx: State<'_, AppContext>, plugin_id: String) -> AppResult<()> {
    ctx.plugin_manager.start(&plugin_id).await.map_err(safe)
}
/// Revoke API authority before removing subscription and waiting for engine release.
#[tauri::command]
pub async fn plugin_stop(ctx: State<'_, AppContext>, plugin_id: String) -> AppResult<()> {
    ctx.plugin_manager.stop(&plugin_id).await.map_err(safe)
}
/// Wait for old worker exit, then revalidate the code and configuration.
#[tauri::command]
pub async fn plugin_reload(ctx: State<'_, AppContext>, plugin_id: String) -> AppResult<()> {
    ctx.plugin_manager.reload(&plugin_id).await.map_err(safe)
}
/// Generic reconciliation prompt; never a replay or balloon-specific action.
#[tauri::command]
pub async fn plugin_resync(ctx: State<'_, AppContext>, plugin_id: String) -> AppResult<()> {
    ctx.plugin_manager.resync(&plugin_id).map_err(safe)
}
