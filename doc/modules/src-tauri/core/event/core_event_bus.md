# core_event_bus

## 职责
`CoreEventBus` —— 进程内事件总线的**唯一**底座，建立在 `tokio::sync::broadcast` 之上。职责被刻意压到最小：创建 channel（统一容量）、发布事件、创建 receiver、统一的关闭/无消费者日志。

**不负责**：订阅表、同步回调、延迟队列、自建分发器 —— 重新实现任何一项都会变成与 `broadcast` 等价的第二套分发系统。

## 核心类型/函数
- **`CORE_EVENT_CHANNEL_CAPACITY`**（1024）— 每个 receiver 的待消费上限。取值权衡：事件是轻量事实通知，正常消费远快于产生；容量只需覆盖「消费者短暂卡顿」的窗口。过大只会把内存占用与「消费者已严重落后」的发现时间一起推后。
- **`CoreEventBus`** — 持有 `broadcast::Sender<CoreEvent>`。不实现 `Clone`（用 `Arc<CoreEventBus>` 共享）：多份 sender 会让「谁在发布事件」变得不可推理。
- **`CoreEventBus::new()` / `with_capacity(capacity)`** — 容量为 0 时钳到 1（`broadcast` 不接受 0）。初始 receiver 立即丢弃：订阅一律由消费者显式 `subscribe()`，避免「总线自己持有一个永不消费的 receiver」把 `send` 的语义搞糊。
- **`publish(&self, event: CoreEvent)`** — **同步非阻塞**（`Sender::send` 本身不需要 `await`，故同步 Service 的方法签名不必改为 async）。事件名在 `send` 之前取（`send` 会消费掉事件）。
- **`subscribe(&self) -> broadcast::Receiver<CoreEvent>`** — receiver 只收到**订阅之后**发布的事件。
- **`receiver_count(&self) -> usize`** — 诊断 / 测试用。
- **`Default`** — 默认实现（等价 `new()`）。

## 直接依赖
- `tokio::sync::broadcast`
- `tracing::debug`
- `core::event::core_event::CoreEvent`

## 被依赖
- `core::context::AppContext`（持有 `Arc<CoreEventBus>` 并注入各 Service）
- 全部 Service 与适配器（发布事实通知）
- `core::event::consumer::spawn_consumer`（取 receiver）
- `main.rs`（前端桥 / 审计 / 插件宿主三个消费者）
- `plugin::host::plugin_host::PluginHost`（只持有句柄，不建第二套总线）

## 逻辑流程
**publish**：取 `event.kind()` → `tx.send(event)` → `Ok(receivers)` 记 `debug`；`Err` 表示「无消费者」，同样只记 `debug`。

## 语义（务必读）
```text
broadcast::send() 成功
  ≠ 所有消费者已经处理完成
  ≠ 事件永久可靠送达
  ≠ 业务动作已经执行成功
```
- **无 receiver 时 `send` 返回 `Err`** —— 这不是业务失败，只记日志；业务动作在发布**之前**就必须已完成，发布失败不回滚业务动作。
- 消费者落后时收到 `Lagged`，必须查询当前状态重新同步（见 `consumer`）。
- 容量有限：不追求「不丢事件」，只追求「丢得可见、丢后可恢复」。

## 测试
`tests/core_event_bus_tests.rs`：多 receiver 扇出、无 receiver 发布不失败、发布同步非阻塞（10k 次发布 < 1s）、订阅者收不到历史事件、慢 receiver 观测到 `Lagged`、总线析构后 receiver 收到 `Closed`、容量 0 被钳制、`receiver_count` 真实、`Arc` 可跨 task 共享。
