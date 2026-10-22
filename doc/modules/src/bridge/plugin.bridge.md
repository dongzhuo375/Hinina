# plugin.bridge

`pluginBridge` 通过统一 `ipcInvoke` 调用 plugin_list/start/stop/reload/resync，只传插件 ID；订阅 `plugin-notification` 返回卸载函数。没有文件或网络操作。

插件命令直接拒绝稳定 `{code, permission?}`；统一出口归一化为 `IpcError`，保留 `.code` 与 `.raw`。这些错误不会被当作 `Auth` 而触发登出。
