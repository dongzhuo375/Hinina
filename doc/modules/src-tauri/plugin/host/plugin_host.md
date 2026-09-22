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
- **`trait PluginEventSink: Send + Sync`** — 插件事件投递出口，唯一方法 `deliver(&self, envelope: &PluginEventEnvelope)`。契约三条：**不得 panic**（投递失败自行吞掉并记录，宿主也会隔离单个插件的异常）、**应为非阻塞**、**不得同步重入宿主**（如在 `deliver` 内调用 `PluginHost::subscribe` / `unsubscribe`，或经任何路径再次触发投递 —— 宿主虽已在**无锁上下文**调用本方法（不会死锁），同步重入仍会造成无限递归；需要响应事件时把信封转入插件自己的队列，由插件侧事件循环异步处理）。**`tokio::sync::mpsc::UnboundedSender<PluginEventEnvelope>` 已直接实现**（通道关闭 = 插件已卸载或崩溃，只记 `debug`）
- **`PluginHostError`**（`thiserror`，`PartialEq + Eq`）— `PermissionDenied { plugin_id }`（manifest 未声明 `Notification`）/ `AlreadySubscribed { plugin_id }`（同一插件重复订阅）
- **`Subscription`**（私有）— 单个插件的订阅项：`adapter: PluginEventAdapter`（序号按订阅独立递增）、`sink: Arc<dyn PluginEventSink>`
- **`SubscriptionTable`**（私有类型别名）— `Arc<Mutex<HashMap<String, Subscription>>>`。抽出来是为了让消费者闭包**只捕获这张表**而不是整个宿主
- **`PluginHost`** — 字段：`bus: Arc<CoreEventBus>`（仅用于启动消费者取一条 receiver）、`subscriptions: SubscriptionTable`（锁内只做「适配 + 收集」，**投递在锁外**）
  - `new(bus: Arc<CoreEventBus>) -> Self`（`#[must_use]`）— 创建宿主（不订阅、不启动消费者）
  - `subscribe(&self, manifest: &PluginManifest, sink: Arc<dyn PluginEventSink>) -> Result<(), PluginHostError>` — 权限判据是 manifest 声明了 `PluginPermission::Notification`（事件投递是「通知」能力，不能借 `ContestRead` 之类的读取权限顺带获得）；重复订阅返回 `AlreadySubscribed`。流程：**先无锁做一次重复检查**（明确被拒的订阅不下发通知）→ 构造 `Subscription`（序号从 0 起）→ **在取锁之前**投递初始 `ResyncRequired{lost:0}` → 取锁**复查**同 id → `insert`。取锁前投递有两个不可省的理由：① `deliver` 是用户实现，持锁调用时实现内回调宿主的任何方法（都取同一把不可重入锁）即**自死锁**，必须与 `dispatch_to_subscribers` 的「锁内收集、锁外投递」保持同一契约；② 通知必须先于任何经消费者分发的事件到达 sink（否则插件先看到 `seq=2` 的事件、再看到 `seq=1` 的通知，形成假缺口），而 dispatch 只能经锁看到订阅表，**插入前完成投递即保证顺序**。极端并发下同 id 抢插时落败方会先多收一条通知、再拿到 `AlreadySubscribed`（无害的「请先同步」提示）
  - `unsubscribe(&self, plugin_id: &str) -> bool` — 注销订阅（**幂等**），返回此前是否存在
  - `subscriber_count(&self) -> usize`（`#[must_use]`）— 诊断 / 测试用
  - `start(self: &Arc<Self>) -> tokio::task::JoinHandle<ConsumerExit>` — 启动宿主消费者。**只把订阅表（`Arc::clone(&self.subscriptions)`）交给闭包**；`Lagged` 时向所有插件下发 `ResyncRequired`（`ResyncReason::Startup` 折算为 `lost = 0`）。注意：`start()` **不**补发初始通知 —— 初始同步提示由 `subscribe()` 负责（订阅即下发），因此晚于 `start()` 订阅的插件也不会错过它；早于 `start()` 订阅的插件会先后收到两条 `lost:0`（订阅时 + Startup），属幂等提示
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

插件侧（任意时刻，可晚于 start()）：
  → PluginHost::subscribe(manifest, sink)
      ① 无锁重复检查 → ② 构造 Subscription（seq=0）→ ③ **无锁** sink.deliver(ResyncRequired{lost:0}, seq=1)
      → ④ 取锁复查同 id → ⑤ insert（此后消费者才可能把事件分发到本插件，故 seq≥2）
```

## 设计约束
- **不形成「消费者 → 宿主 → 总线」强引用环**：消费者闭包捕获的是订阅表 `Arc<Mutex<HashMap<..>>>`，**不是 `Arc<PluginHost>`** —— 后者会让总线永不析构、`ConsumerExit::Closed` 永不发生（进程退出时消费者只能被运行时强杀）。
- **投递一律在锁外**：慢插件不得阻塞其他插件的投递，也不得阻塞新插件注册。`dispatch_to_subscribers` / `notify_resync` 先在锁内 `filter_map` 出 `(sink, envelope)` 列表、出锁后再逐条 `deliver`；`subscribe` 的初始通知同样**在取锁之前**投递（`deliver` 是用户实现，持锁调用会自死锁 —— 见 `subscribe` 条目）。
- **异常隔离**：单个插件的 sink 投递失败（或 panic）不影响其他插件，也不影响宿主与发布方 —— 事件消费本就与业务动作无关。
- **权限不可绕过**：`subscribe` 是唯一的注册入口，且必须显式声明 `Notification`。
- **锁中毒统一 `into_inner` 取回内部数据**：订阅表 panic 不会让结构不一致，而「静默失败」会让插件永久收不到事件。

## 测试
`tests/plugin_host_tests.rs`：订阅须有 `Notification` 权限（否则 `PermissionDenied` 且不注册）、重复订阅被拒、`unsubscribe` 幂等且停止投递、宿主过滤白名单外事件、**序号按订阅独立**、**单插件 sink 失败不影响其他插件**、经总线驱动投递（`host_consumer_delivers_from_bus`）、`Lagged` 后向插件下发 `ResyncRequired`、**晚于 `start()` 订阅也收到初始同步提示**（`subscribe_after_start_receives_initial_notice`：金丝雀插件把「晚订阅」时序钉成确定性，`sequences() == [1, 2]` 锁定无假缺口）、**`deliver` 内回调宿主不死锁**（`reentrant_sink_does_not_deadlock`：带超时信道把「挂住」转成测试失败 —— 还原「锁内投递」后该用例必然超时失败）、`host_does_not_own_a_second_bus`（宿主只借用总线，不自建事件源）。
