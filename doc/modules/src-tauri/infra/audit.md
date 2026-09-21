# audit

## 职责
审计 / 日志消费者 —— 核心事件的**只读**观察者。存在的意义有两个：① 排障与审计：所有核心事实集中在一处按事件名结构化落日志，不必到各 Service 里翻日志拼时间线；② 结构验证：它是「同一底层事件流可挂多个互不相干的消费者」的**在产证据** —— 与 Tauri 前端桥、PluginHost 平级，谁也不依赖谁。

## 核心类型/函数
- **`AUDIT_CONSUMER_NAME: &str = "audit"`** — 审计消费者的稳定名（日志字段）
- **`spawn_audit_consumer(bus: &Arc<CoreEventBus>) -> tokio::task::JoinHandle<ConsumerExit>`** — 经 `core::event::consumer::spawn_consumer` 启动消费者；`handle` 同步调用 `log_event` 并返回 `std::future::ready(())`；`resync` 回调**只在 `ResyncReason::Lagged` 时记一条 `debug`** —— 审计只读、不承担任何状态同步职责，落后无需重新查询任何东西。返回的 `JoinHandle` 产出 `ConsumerExit`，供组合根在停机时观察
- **`log_event(event: &CoreEvent)`**（私有）— 按事件类型记录审计日志。分级依据「排障价值 / 频率」：
  - `info`：低频且对复盘有意义的事实 —— `LoggedIn`（带 `user_id`）/ `LoggedOut` / `SessionExpired` / `TokenRotated` / `OjSwitched` / `WorkspaceSaved`（带 `workspace_id` / `revision` / `automatic`）/ `SubmissionCreated` / `SubmissionJudged`（带 `status`）/ `AnnouncementChanged`（只记 `new_count = new_ids.len()`）/ `ContestSelected` / `ConfigChanged`
  - `debug`：可能高频的纯状态提醒 —— `ProblemOpened`、`ThemeChanged`
  - 每条日志都带 `event = event.kind()` 字段；**只记录事件名与 ID / 状态摘要**，不额外拼接领域实体（`CoreEvent` 载荷本身已无敏感字段）

## 直接依赖
- `std::sync::Arc`
- `tracing::{debug, info}`
- `core::event::consumer::{spawn_consumer, ConsumerExit, ResyncReason}`
- `core::event::core_event::CoreEvent`
- `core::event::core_event_bus::CoreEventBus`

## 被依赖
- `main.rs`（`spawn_event_consumers` 内与前端桥、PluginHost 一同启动）

## 逻辑流程
```
main.rs::spawn_event_consumers
  → spawn_audit_consumer(&bus)
      → spawn_consumer(bus, "audit", |e| { log_event(&e); ready(()) }, |reason| { Lagged 才 debug })
          ├─ 事件到达 → log_event：match 变体 → info!/debug!(event = kind(), …)
          ├─ Lagged   → debug!("审计消费者落后（只读消费者，无需重新同步）")
          └─ Closed   → 消费者干净退出（ConsumerExit::Closed）
```

## 设计约束
- **只读**：不做任何持久化、不调用 Service、不改变任何状态。因此它落后、崩溃或缺失都不影响任何业务动作（`CoreEventBus::publish` 在无消费者时也只是记一条 `debug`）。
- **它是「消费者平级」的结构证据**：三个消费者各自只从同一条 broadcast 流取自己的 receiver，互不依赖；新增消费者只需再调一次 `spawn_consumer`，不必改动任何 Service。
- **不记录凭证内容**：`TokenRotated` 只记「已轮换」这一事实（`oj_id`），token 本身不在载荷里，此处也不额外拼接。

## 测试
`tests/audit_tests.rs`：`audit_consumer_handles_every_variant` —— 逐变体发布并断言消费者全部处理完毕（新增事件变体时若忘了在 `log_event` 中 match，用例会失败，防止事件在审计路径上静默消失）；`publishing_without_audit_consumer_is_fine` —— 未启动审计消费者时发布仍成功（只读观察者的缺席不影响发布方）。
