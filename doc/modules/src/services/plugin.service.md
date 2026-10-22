# plugin.service

`pluginService` 是插件开发控制与纯文本通知的前端服务入口，委托 pluginBridge。Store 不直接调用 Tauri，Service 不绕过 Rust 做文件或网络操作。命令失败原样交给统一 IPC 错误契约处理。

subscribeNotifications(onAdd, onDismiss) 验证 pluginId、消息长度和级别，分配内部ID，并负责最多5项、6秒过期策略。返回可调用的卸载句柄，其 dismiss(id) 同时释放队列名额和定时器。卸载和订阅失败清理全部定时器；卸载后忽略迟到事件。Service 只管理订阅策略，不持有Pinia状态；Store应用add/dismiss事件。

subscribeNotifications 同时负责"单一活动订阅"：新的调用先卸载旧订阅（解除监听并清空其可见通知），并推进单调代次；在 await 期间被更新调用取代的订阅在完成后立即卸载，不会成为活动订阅，也不接管 dismiss。返回的卸载函数只在自己仍是活动订阅时生效。跨层共享的 dismissNotification(id) 只作用于当前活动订阅。
