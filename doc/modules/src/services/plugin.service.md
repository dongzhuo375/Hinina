# plugin.service

`pluginService` 是插件开发控制与纯文本通知的前端服务入口，委托 pluginBridge。Store 不直接调用 Tauri，Service 不绕过 Rust 做文件或网络操作。命令失败原样交给统一 IPC 错误契约处理。

subscribeNotifications(onAdd, onDismiss) 验证 pluginId、消息长度和级别，分配内部ID，并负责最多5项、6秒过期策略。返回可调用的卸载句柄，其 dismiss(id) 同时释放队列名额和定时器。卸载和订阅失败清理全部定时器；卸载后忽略迟到事件。Service 只管理订阅策略，不持有Pinia状态；Store应用add/dismiss事件。
