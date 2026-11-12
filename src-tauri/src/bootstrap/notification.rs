// 插件通知的前端出口（组合根唯一的前端通知路径）。
//
// 原住在 `infra::plugin_notification` —— 那让它成为 infra 层**唯一的**框架
// （tauri）依赖，与「infra 传输无关」的红线相抵。端口
// `NotificationOutput`（`plugin::execution_api`）本身是干净的，脏的只是
// 实现的位置：本类型只被 `setup::install` 的 `set_output` 使用，理应与前端
// 事件桥同住组合根。宿主级通知失败经 `plugin_diagnostics::failure` 走统一形状。

use tauri::{Emitter, Manager};

use crate::infra::plugin_diagnostics;
use crate::plugin::api::notification::{validate_message, PluginNotificationLevel};
use crate::plugin::api::{PluginError, PluginResult};
use crate::plugin::execution_api::NotificationOutput;

pub(crate) struct TauriPluginNotifications(pub(crate) tauri::AppHandle);

impl NotificationOutput for TauriPluginNotifications {
    fn emit(
        &self,
        plugin_id: &str,
        level: PluginNotificationLevel,
        message: &str,
    ) -> PluginResult<()> {
        validate_message(message)?;
        if self.0.webview_windows().is_empty() {
            plugin_diagnostics::failure(Some(plugin_id), "notification.emit", "no_window");
            return Err(PluginError::Unavailable);
        }
        self.0
            .emit(
                "plugin-notification",
                serde_json::json!({"pluginId":plugin_id,"level":level,"message":message}),
            )
            .map_err(|_| {
                plugin_diagnostics::failure(Some(plugin_id), "notification.emit", "emit_failed");
                PluginError::Unavailable
            })
    }
}
