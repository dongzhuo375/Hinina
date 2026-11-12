# bootstrap/notification

## 职责
插件通知的前端出口 —— `NotificationOutput` 端口的 Tauri 实现，也是组合根唯一的前端通知路径。**必须留在组合根**：原住 `infra::plugin_notification`，那让它成为 infra 层唯一的框架（tauri）依赖，与「infra 传输无关」的红线相抵；端口 `NotificationOutput`（`plugin::execution_api`）本身是干净的，脏的只是实现位置。

## 核心类型/函数
- `struct TauriPluginNotifications(tauri::AppHandle)` — 由 `bootstrap::setup::install` 经 `plugin_manager.set_output` 注入。
- `impl NotificationOutput for TauriPluginNotifications` — `emit(plugin_id, level, message)`：
  1. `validate_message(message)`（长度/字符约束，失败原样返回插件错误）；
  2. 无 webview 窗口 → `plugin_diagnostics::failure(..., "no_window")` + `PluginError::Unavailable`；
  3. `emit("plugin-notification", {pluginId, level, message})`；失败 → `plugin_diagnostics::failure(..., "emit_failed")` + `PluginError::Unavailable`。

## 直接依赖
- `crate::plugin::api::notification::{validate_message, PluginNotificationLevel}`
- `crate::plugin::api::{PluginError, PluginResult}`
- `crate::plugin::execution_api::NotificationOutput`
- `crate::infra::plugin_diagnostics::failure`
- `tauri::{AppHandle, Emitter, Manager}`、`serde_json`

## 被依赖
- `crate::bootstrap::setup::install`（`set_output`）

## 逻辑流程
插件宿主产生通知 → `NotificationOutput::emit` → 校验 → 无窗口/发送失败记统一形状的诊断并返回 `Unavailable`；成功时前端经 `plugin-notification` 通道收到 `{pluginId, level, message}`。
