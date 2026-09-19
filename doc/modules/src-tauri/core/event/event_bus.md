# event_bus

## 职责
实现应用事件总线 `EventBus`，支持按 `EventCategory` 分类订阅与发布事件。订阅者按类别过滤，避免收到不关心的事件；订阅 `All` 类别的订阅者接收所有事件。

**两种投递模式**（发布方按订阅者的性质选择）：
- **同步投递**（`subscribe`）：回调在 `publish` 的调用栈内执行 —— 只放纯内存处理（清缓存、置标志），**禁止 I/O**。
- **延迟投递**（`subscribe_deferred`）：回调由专用后台线程执行，`publish` 立即返回 —— 用于磁盘清理等 I/O。使用前提：该 handler **不承担正确性职责**（正确性须由结构保证，例如缓存键自带作用域，而不是依赖「清理及时」），且进程退出时可能尚未执行。

## 核心类型/函数
- **`SubscriptionId`** — 订阅 ID 类型别名（`usize`）
- **`EventHandler`** — 事件处理回调类型（`Arc<dyn Fn(&AppEvent) + Send + Sync>`），接收事件引用
- **`Subscriber`**（内部）— 一个订阅项：`id` + `handler` + `deferred` 投递模式
- **`DeferredQueue`**（内部）— 延迟投递队列：**专用后台线程 + `std::sync::mpsc` 无界通道**消费。不用 `tokio::spawn` 的原因：`publish` 会被同步上下文调用（`WorkspaceManager::save`、纯 `#[test]`），那里没有 tokio runtime，`tokio::spawn` 会 panic；线程创建失败时退化为「延迟订阅不投递」并告警（不让发布方因投递设施故障而失败）。线程随 `EventBus` 生命周期存在（析构时通道关闭、线程自然退出），不做 join（避免 handler 阻塞时卡住析构）
- **`EventBus`** — 事件总线 struct，内部持有 `RwLock<HashMap<EventCategory, Vec<Subscriber>>>`、`Mutex<SubscriptionId>` 与 `DeferredQueue`
- **`EventBus::new()`** — 创建新 EventBus（含延迟投递线程）
- **`EventBus::publish(&self, event: &AppEvent)`** — 发布事件，通过 `AppEvent::category()` 映射类别，分发到对应类别 + All 订阅者。**同步订阅者**在本调用栈内执行（`catch_unwind` 隔离 panic）；**延迟订阅者**的事件被克隆进任务投递到后台线程后立即返回 —— 发布方不为订阅者的 I/O 买单
- **`EventBus::subscribe(&self, category, handler) -> SubscriptionId`** — 注册**同步**订阅
- **`EventBus::subscribe_deferred(&self, category, handler) -> SubscriptionId`** — 注册**延迟**订阅
- **`EventBus::unsubscribe(&self, id)`** — 全局遍历 retain 移除匹配 ID（两种投递模式共用），清理空类别
- **`EventBus::flush_deferred(&self)`** — **仅测试用**（`#[cfg(test)]`）：投递哨兵任务并等待其完成，即「等延迟队列排空」。队列 FIFO 且消费线程串行，故哨兵完成 = 此前投递的任务都已执行；测试用它避免「断言早于延迟 handler」的偶发失败
- **`Default for EventBus`** — 默认实现

## 直接依赖
- `core::event::app_event::AppEvent`
- `core::event::event_category::EventCategory`
- `std::collections::HashMap`
- `std::sync::{Arc, Mutex, RwLock}`
- `std::sync::mpsc::{self, Sender}`
- `std::panic::AssertUnwindSafe`
- `tracing::{debug, warn}`

## 被依赖
- `core::context`（AppContext 持有 Arc<EventBus>）
- `service::workspace::manager`（发布 Workspace 事件；`Saved`/`AutoSaveTriggered` 由 `main.rs` 桥接前端）
- `service::{contest,problem,submission}`（订阅 `SystemEvent::OJSwitched`：**内存段同步**清缓存，**磁盘段延迟**清理）
- `service::auth`（同步订阅 `AuthEvent::TokenRefreshed` 回写磁盘会话 —— 刻意保持同步：会话落盘是持久性保证，且需与登出的删除操作保持顺序）
- `main.rs`（同步订阅 Workspace 类别 → `emit` 到 webview）

## 逻辑流程
- **publish**：`AppEvent::category()` 映射类别 → 读锁收集该类别 + All 的订阅者并按投递模式分组 → 锁外先跑同步订阅者（逐个 `catch_unwind` 隔离）→ 再把事件克隆进任务逐个投递给延迟队列 → 返回
- **subscribe / subscribe_deferred**：Mutex 分配递增 ID（wrapping_add 防溢出），写锁插入 subscribers map，标记投递模式
- **unsubscribe**：写锁遍历所有类别 retain 移除匹配 ID，保留非空类别
- **延迟消费线程**：`recv` 取任务 → `catch_unwind` 执行（panic 不终止消费线程）→ 通道关闭（总线析构）即退出

## 测试
测试代码位于 `tests/event_bus_tests.rs`，15 项：分类发布、All 通配、取消订阅、无订阅者不 panic、多订阅者、唯一 ID、空类别清理、事件数据正确性、handler panic 隔离；延迟投递 6 项——事件最终送达、**发布方不被订阅者 I/O 阻塞**（订阅者阻塞 1s 而 `publish` < 300ms）、同步先于 `publish` 返回而延迟随后、类别过滤、**延迟 handler panic 不终止消费线程**、取消订阅后不再投递。
