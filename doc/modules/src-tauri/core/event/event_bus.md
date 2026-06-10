# event_bus

## 职责
实现应用事件总线 `EventBus`，支持按 `EventCategory` 分类订阅与发布事件。订阅者按类别过滤，避免收到不关心的事件；订阅 `All` 类别的订阅者接收所有事件。

## 核心类型/函数
- **`SubscriptionId`** — 订阅 ID 类型别名（`usize`）
- **`EventHandler`** — 事件处理回调类型（`Arc<dyn Fn(AppEvent) + Send + Sync>`）
- **`EventBus`** — 事件总线 struct，内部持有 `RwLock<HashMap<EventCategory, Vec<(SubscriptionId, EventHandler)>>>` 和 `Mutex<SubscriptionId>`
- **`EventBus::new()`** — 创建新 EventBus
- **`EventBus::publish(&self, event: AppEvent)`** — 发布事件，通知匹配类别及 All 订阅者（TODO）
- **`EventBus::subscribe(&self, category, handler) -> SubscriptionId`** — 订阅事件（TODO）
- **`EventBus::unsubscribe(&self, id)`** — 取消订阅（TODO）
- **`Default for EventBus`** — 默认实现

## 直接依赖
- `core::event::app_event::AppEvent`
- `core::event::event_category::EventCategory`
- `std::collections::HashMap`
- `std::sync::{Arc, Mutex, RwLock}`

## 被依赖
- `core::context`（AppContext 持有 Arc<EventBus>）
- `service::workspace::manager`

## 逻辑流程
`publish` 遍历 `subscribers` 中匹配类别及 `All` 类别的 handler 并逐一调用；`subscribe` 分配递增 ID 并插入对应类别列表；`unsubscribe` 按 ID 移除。三个方法当前均为 TODO 占位。
