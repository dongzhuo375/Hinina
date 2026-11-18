# plugin.bridge

`pluginBridge` 通过统一 `ipcInvoke` 调用 plugin_list/start/stop/reload/resync，只传插件 ID；订阅 `plugin-notification` 返回卸载函数。没有文件或网络操作。

`onNotification` 在 bridge 层校验载荷**形状**（payload 为对象且 pluginId/message/level 均为 string，否则整事件丢弃——与 announcement/workspace 事件桥同一口径，越界数据不进 store）；长度上限/级别白名单等域规则由 `plugin.service` 的 `validNotification` 兜底。

插件命令直接拒绝稳定 `{code, permission?}`；统一出口归一化为 `IpcError`，保留 `.code` 与 `.raw`。这些错误不会被当作 `Auth` 而触发登出。
