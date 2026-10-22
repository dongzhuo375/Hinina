//! @plugin-module
//! @layer infra/plugin_notification
//! @version 2
//! @stability stable
//! Text-only frontend output. Rust and frontend both bound the payload.
use super::plugin_diagnostics as diagnostics;
use crate::plugin::{
    api::{notification::*, *},
    execution_api::NotificationOutput,
};
use tauri::{Emitter, Manager};

/// @plugin-api
/// @since 2.0
/// @permission host-only
/// @stability stable
pub struct TauriPluginNotifications(pub tauri::AppHandle);
impl NotificationOutput for TauriPluginNotifications {
    fn emit(
        &self,
        plugin_id: &str,
        level: PluginNotificationLevel,
        message: &str,
    ) -> PluginResult<()> {
        validate_message(message)?;
        if self.0.webview_windows().is_empty() {
            diagnostics::failure(Some(plugin_id), "notification.emit", "no_window");
            return Err(PluginError::Unavailable);
        }
        self.0
            .emit(
                "plugin-notification",
                serde_json::json!({"pluginId":plugin_id,"level":level,"message":message}),
            )
            .map_err(|_| {
                diagnostics::failure(Some(plugin_id), "notification.emit", "emit_failed");
                PluginError::Unavailable
            })
    }
}
