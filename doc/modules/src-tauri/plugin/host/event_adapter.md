# host/event_adapter

## 职责
CoreEvent → PluginEvent 的唯一白名单/裁剪点：不创建 channel、不持有 Service。

## 核心类型/函数
- **`PluginEventAdapter`** — 每订阅一个实例：
  - `adapt_if(event)` — 先投影，再按 Host 授权/声明筛选，再递增每订阅 sequence；过滤不会产生假缺口
  - `adapt(event)` — 不附加订阅筛选的底层投影入口（Host 生产路径只用 `adapt_if`）
  - `resync_notice(lost)` — 生成强制控制通知
  - `wrap(event)`（私有）— 组装 `PluginEventEnvelope{version, sequence, occurredAt}`

## 直接依赖
- `core::event::core_event::CoreEvent`
- `plugin::api::event::{PluginEvent, PluginEventEnvelope, PLUGIN_EVENT_PROTOCOL_VERSION}`

## 被依赖
- `plugin::host::plugin_host`（投递路径）、`plugin::manager`（allowlist 生成）

## 逻辑流程
- **裁剪原则**：TokenRotated 始终返回 None；登录丢弃用户 ID；公告只给数量；工作区不带路径或内容
- **序号**：每订阅从 1 起、重新注册重置，不是全局持久序号
- **信封**：version=2，type 与 Manifest 点分事件名一致；`occurredAt` 为适配时刻（UTC 毫秒，`wrap()` 内 `utc_now_ms()`），不可当作提交时间
- **协议纪律**：新事件必须显式加入独立协议和白名单，不得直接序列化 CoreEvent
- 测试锁定所有变体投影、终态摘要、版本和序号、过滤、TokenRotated 排除以及敏感字段不进入协议
