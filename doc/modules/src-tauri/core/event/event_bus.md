# event_bus

## 职责
实现应用事件总线 `EventBus`，支持按 `EventCategory` 分类订阅与发布事件。订阅者按类别过滤，避免收到不关心的事件；订阅 `All` 类别的订阅者接收所有事件。

## 核心类型/函数
- **`SubscriptionId`** — 订阅 ID 类型别名（`usize`）
- **`EventHandler`** — 事件处理回调类型（`Arc<dyn Fn(&AppEvent) + Send + Sync>`），接收事件引用
- **`EventBus`** — 事件总线 struct，内部持有 `RwLock<HashMap<EventCategory, Vec<(SubscriptionId, EventHandler)>>>` 和 `Mutex<SubscriptionId>`
- **`EventBus::new()`** — 创建新 EventBus
- **`EventBus::publish(&self, event: AppEvent)`** — 发布事件，通过 `AppEvent::category()` 映射类别，分发到对应类别 + All 订阅者。回调在锁外执行防死锁
- **`EventBus::subscribe(&self, category, handler) -> SubscriptionId`** — 分配递增 ID，写入对应类别列表
- **`EventBus::unsubscribe(&self, id)`** — 全局遍历 retain 移除匹配 ID，清理空类别
- **`Default for EventBus`** — 默认实现

## 直接依赖
- `core::event::app_event::AppEvent`
- `core::event::event_category::EventCategory`
- `std::collections::HashMap`
- `std::sync::{Arc, Mutex, RwLock}`
- `tracing::debug`

## 被依赖
- `core::context`（AppContext 持有 Arc<EventBus>）
- `service::workspace::manager`

## 逻辑流程
- **publish**：事件通过 `AppEvent::category()` 映射到 EventCategory，读锁收集对应类别 + All 的 handler，锁外逐一调用（避免回调中操作 EventBus 死锁）
- **subscribe**：Mutex 分配递增 ID（wrapping_add 防溢出），写锁插入 subscribers map
- **unsubscribe**：写锁遍历所有类别 retain 移除匹配 ID，保留非空类别

## 测试
测试代码位于 `tests/event_bus_tests.rs`，8 项：分类发布、All 通配、取消订阅、无订阅者不 panic、多订阅者、唯一 ID、空类别清理、事件数据正确性。
