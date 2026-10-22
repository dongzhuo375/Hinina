//! Developer plugin controls. These commands never accept permissions, source, URLs or file paths.
use crate::{
    core::context::AppContext,
    plugin::{api::PluginError, manager::PluginStatus},
};
use tauri::State;

/// List configured plugin status using stable, redacted diagnostics.
#[tauri::command]
pub async fn plugin_list(ctx: State<'_, AppContext>) -> Result<Vec<PluginStatus>, PluginError> {
    let manager = ctx.plugin_manager.clone();
    tokio::task::spawn_blocking(move || manager.list())
        .await
        .map_err(|_| PluginError::Unavailable)?
}
/// Start only a plugin explicitly enabled and approved in the host configuration.
#[tauri::command]
pub async fn plugin_start(
    ctx: State<'_, AppContext>,
    plugin_id: String,
) -> Result<(), PluginError> {
    ctx.plugin_manager.start(&plugin_id).await
}
/// Revoke API authority before removing subscription and waiting for engine release.
#[tauri::command]
pub async fn plugin_stop(ctx: State<'_, AppContext>, plugin_id: String) -> Result<(), PluginError> {
    ctx.plugin_manager.stop(&plugin_id).await
}
/// Wait for old worker exit, then revalidate the code and configuration.
#[tauri::command]
pub async fn plugin_reload(
    ctx: State<'_, AppContext>,
    plugin_id: String,
) -> Result<(), PluginError> {
    ctx.plugin_manager.reload(&plugin_id).await
}
/// Generic reconciliation prompt; never a replay or balloon-specific action.
#[tauri::command]
pub async fn plugin_resync(
    ctx: State<'_, AppContext>,
    plugin_id: String,
) -> Result<(), PluginError> {
    ctx.plugin_manager.resync(&plugin_id)
}
