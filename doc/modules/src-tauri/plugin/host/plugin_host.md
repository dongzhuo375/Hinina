# plugin_host

## 职责
`PluginHost` —— 插件事件适配、权限、脱敏与生命周期管理。

**定位（务必读）**：`PluginHost` **不是第二套业务总线**。它不创建自己的 channel、不维护自己的事件源；只从 `CoreEventBus` 取一条 receiver（经 `core::event::consumer::spawn_consumer`），与 Tauri 前端桥、审计消费者**平级**；它把 `CoreEvent` 交给每个插件各自的 `PluginEventAdapter` 转换后投递。

| 事项 | 落点 |
|---|---|
| 事件白名单、字段裁剪、脱敏、协议版本、序号 | `plugin::host::event_adapter` |
| 插件权限校验（须声明 `Notification`） | `PluginHost::subscribe` |
| 订阅生命周期（注册 / 注销） | `PluginHost::subscribe` / `PluginHost::unsubscribe` |
| 消费者落后后的重新同步通知 | `ResyncReason` → `PluginEvent::ResyncRequired` |
| 单个插件异常隔离 | 每个插件独立 sink，投递失败只影响该插件 |

v0.x 只提供**架构边界**：没有 JS/WASM 运行时、没有插件目录扫描；`PluginEventSink` 是运行时接入点（`mpsc::UnboundedSender` 已直接可用）。

## 核心类型/函数
- **`PLUGIN_HOST_CONSUMER_NAME: &str = "plugin-host"`** — 消费者稳定名（日志字段）
- **`trait PluginEventSink: Send + Sync`** — 插件事件投递出口，唯一方法 `deliver(&self, envelope: &PluginEventEnvelope)`（应为非阻塞，实现**不得 panic**：投递失败自行吞掉并记录，宿主也会隔离单个插件的异常）。**`tokio::sync::mpsc::UnboundedSender<PluginEventEnvelope>` 已直接实现**（通道关闭 = 插件已卸载或崩溃，只记 `debug`）
- **`PluginHostError`**（`thiserror`，`PartialEq + Eq`）— `PermissionDenied { plugin_id }`（manifest 未声明 `Notification`）/ `AlreadySubscribed { plugin_id }`（同一插件重复订阅）
- **`Subscription`**（私有）— 单个插件的订阅项：`adapter: PluginEventAdapter`（序号按订阅独立递增）、`sink: Arc<dyn PluginEventSink>`
- **`SubscriptionTable`**（私有类型别名）— `Arc<Mutex<HashMap<String, Subscription>>>`。抽出来是为了让消费者闭包**只捕获这张表**而不是整个宿主
- **`PluginHost`** — 字段：`bus: Arc<CoreEventBus>`（仅用于启动消费者取一条 receiver）、`subscriptions: SubscriptionTable`（锁内只做「适配 + 收集」，**投递在锁外**）
  - `new(bus: Arc<CoreEventBus>) -> Self`（`#[must_use]`）— 创建宿主（不订阅、不启动消费者）
  - `subscribe(&self, manifest: &PluginManifest, sink: Arc<dyn PluginEventSink>) -> Result<(), PluginHostError>` — 权限判据是 manifest 声明了 `PluginPermission::Notification`（事件投递是「通知」能力，不能借 `ContestRead` 之类的读取权限顺带获得）；重复订阅返回 `AlreadySubscribed`
  - `unsubscribe(&self, plugin_id: &str) -> bool` — 注销订阅（**幂等**），返回此前是否存在
  - `subscriber_count(&self) -> usize`（`#[must_use]`）— 诊断 / 测试用
  - `start(self: &Arc<Self>) -> tokio::task::JoinHandle<ConsumerExit>` — 启动宿主消费者。**只把订阅表（`Arc::clone(&self.subscriptions)`）交给闭包**；`Lagged` 时向所有插件下发 `ResyncRequired`（`ResyncReason::Startup` 折算为 `lost = 0`，订阅建立时也下发一次，提示插件先同步）
  - `dispatch(&self, event: &CoreEvent)`（`#[cfg(test)]`）— 直接驱动投递逻辑，仅供测试（生产路径一律经 `start()` 的消费者）
- **`dispatch_to_subscribers(subs: &SubscriptionTable, event: &CoreEvent)`**（自由函数）— 锁内只做适配与收集（快），投递在锁外（慢插件不影响别人）
- **`notify_resync(subs: &SubscriptionTable, lost: u64)`**（自由函数）— 向所有插件下发「请重新同步」通知（同样锁内收集、锁外投递）

## 直接依赖
- `std::collections::HashMap`、`std::sync::{Arc, Mutex}`
- `thiserror::Error`
- `tracing::{debug, info, warn}`
- `core::event::consumer::{spawn_consumer, ConsumerExit, ResyncReason}`
- `core::event::core_event::CoreEvent`、`core::event::core_event_bus::CoreEventBus`
- `plugin::api::event::PluginEventEnvelope`
- `plugin::host::event_adapter::PluginEventAdapter`
- `plugin::host::manifest::{PluginManifest, PluginPermission}`

## 被依赖
- `core::context`（`AppContext::plugin_host: Arc<PluginHost>`，初始化序列第 11 步）
- `main.rs`（`spawn_event_consumers` 中调用 `plugin_host.start()`）
- `plugin::host::mod`（`pub mod plugin_host` 声明）

## 逻辑流程
```
AppContext::init → PluginHost::new(bus)（只持句柄）
main.rs::spawn_event_consumers（在 tauri::async_runtime::spawn 内）
  → plugin_host.start()
      → spawn_consumer(&bus, "plugin-host", |e| dispatch_to_subscribers(subs, &e), |r| notify_resync(subs, lost))
          ├─ Startup        → notify_resync(subs, 0)     插件收到 ResyncRequired{lost:0}
          ├─ 事件           → 锁内 adapt 收集 → 锁外 sink.deliver
          ├─ Lagged(lost)   → notify_resync(subs, lost)
          └─ Closed         → ConsumerExit::Closed（总线析构）
```

## 设计约束
- **不形成「消费者 → 宿主 → 总线」强引用环**：消费者闭包捕获的是订阅表 `Arc<Mutex<HashMap<..>>>`，**不是 `Arc<PluginHost>`** —— 后者会让总线永不析构、`ConsumerExit::Closed` 永不发生（进程退出时消费者只能被运行时强杀）。
- **投递在锁外**：慢插件不得阻塞其他插件的投递，也不得阻塞新插件注册。因此 `dispatch_to_subscribers` / `notify_resync` 先在锁内 `filter_map` 出 `(sink, envelope)` 列表，出锁后再逐条 `deliver`。
- **异常隔离**：单个插件的 sink 投递失败（或 panic）不影响其他插件，也不影响宿主与发布方 —— 事件消费本就与业务动作无关。
- **权限不可绕过**：`subscribe` 是唯一的注册入口，且必须显式声明 `Notification`。
- **锁中毒统一 `into_inner` 取回内部数据**：订阅表 panic 不会让结构不一致，而「静默失败」会让插件永久收不到事件。

## 测试
`tests/plugin_host_tests.rs`：订阅须有 `Notification` 权限（否则 `PermissionDenied` 且不注册）、重复订阅被拒、`unsubscribe` 幂等且停止投递、宿主过滤白名单外事件、**序号按订阅独立**、**单插件 sink 失败不影响其他插件**、经总线驱动投递（`host_consumer_delivers_from_bus`）、`Lagged` 后向插件下发 `ResyncRequired`、`host_does_not_own_a_second_bus`（宿主只借用总线，不自建事件源）。
