# plugin 类型

`PluginState` 为 starting/running/stopped/failed；`PluginStatus` 含 id、state、稳定 error（code、可选 permission）。`PluginNotification` 含 pluginId、info/success/warn/error 级别和文本 message。

不携带 Session、Token、源码、路径、Provider 或 JS 异常栈。命令错误由统一 IpcError.code/raw 保留，具体 API 调用类型见 Rust 插件 SDK 的 hinina.d.ts。
