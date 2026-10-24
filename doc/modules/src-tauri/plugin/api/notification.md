# api/notification

## 职责
纯文本用户通知能力（易失提示，非业务凭据）。

## 核心类型/函数
- `MAX_NOTIFICATION_BYTES: usize = 512`
- `PluginNotificationLevel` — info / success / warn / error
- `validate_message(message)` — 文本 1..512 UTF-8 字节，禁止非换行/制表控制字符
- `NotificationApi`（trait）— `notify(level, message)`

## 直接依赖
- `plugin::api::PluginResult`
- `async_trait`、`serde`

## 被依赖
- `plugin::execution_api`（实现：`notification` 权限检查 + 10/min 预算 + `NotificationOutput` 输出）
- `infra::plugin_notification`、前端 Bridge → Service → Store → `PluginNotifications`

## 逻辑流程
- 需要 `notification` 权限；每插件 10 条/分钟；Tauri 输出不存在则 `unavailable`
- 前端用 Vue 文本插值渲染，**不使用 v-html**；同时最多 5 条、6 秒消失、可手动关闭
- 投递是易失的用户提示，不是业务成功凭据；前端尚未监听、窗口关闭或丢失事件时不承诺显示
