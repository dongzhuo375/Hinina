# plugin.service

`pluginService` 是插件开发控制与纯文本通知的前端服务入口，委托 pluginBridge。Store 不直接调用 Tauri，Service 不绕过 Rust 做文件或网络操作。命令失败原样交给统一 IPC 错误契约处理。
