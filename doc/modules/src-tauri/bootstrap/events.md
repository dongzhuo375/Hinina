# bootstrap/events

## 职责
`CoreEvent → webview` 的前端事件桥，以及三个事件消费者的启动。三类消费者共用一条底层事件流：

```text
CoreEventBus（broadcast）
  ├── tauri-frontend-bridge → emit 到 webview（前端 Store 只做 UI 刷新）
  ├── audit                 → 只读审计日志（infra::audit）
  └── plugin-host           → CoreEvent → PluginEvent（白名单/脱敏/版本）
```

内部 Service 之间没有任何事件订阅；事件只承载「已经发生的事实」，消费者落后、崩溃或缺失都不影响业务动作。

## 核心类型/函数
- `fn spawn_event_consumers(app: &tauri::AppHandle)` — 从 `AppContext` 取总线与插件宿主，整段包在 `tauri::async_runtime::spawn` 里（`spawn_consumer` 内部用 `tokio::spawn`，而 setup 回调不在运行时内；**不能**用 setup 内的局部 runtime——它随 setup 结束销毁）。
- `fn spawn_frontend_event_bridge(bus, app)` — `spawn_consumer` + 消费者名 `tauri-frontend-bridge`；`Lagged` 只 `warn!`（前端轮询下一周期补齐）。
- `fn payload_for(&CoreEvent) -> Option<(&'static str, serde_json::Value, &'static str)>` — 纯映射，只转发两种前端需要即时感知的事实：
  - `WorkspaceSaved` → `workspace-saved`，载荷 `{ workspaceId, revision, auto }`（后台 auto-save 是唯一非前端发起的落盘路径，用于清除「编辑中…」指示）；
  - `AnnouncementChanged` → `announcements-published`，载荷 `{ contestId, newIds }`（红点即时点亮）。
  - 其余事件返回 `None`：状态真值经 IPC 查询，转发会制造第二来源。
- `fn emit_or_warn(...)` — `emit` 失败只 `warn!`（事件是刷新触发，丢了不影响正确性）。
- 常量 `WORKSPACE_SAVED_EVENT` / `ANNOUNCEMENTS_PUBLISHED_EVENT` / `FRONTEND_BRIDGE_CONSUMER`。

## 直接依赖
- `crate::core::context::AppContext`
- `crate::core::event::consumer::{spawn_consumer, ResyncReason}`
- `crate::core::event::core_event::CoreEvent`
- `crate::core::event::core_event_bus::CoreEventBus`
- `crate::infra::audit::spawn_audit_consumer`
- `tauri::{AppHandle, Emitter, Manager}`、`serde_json`

## 被依赖
- `crate::bootstrap::setup::install`（消费者在 `manage(AppContext)` 之后启动）

## 逻辑流程
1. `spawn_event_consumers` 投递到 Tauri 运行时后依次启动前端桥、审计消费者、PluginHost；宿主启动失败 `warn!`，最后 `info!`「事件消费者已启动」。
2. 前端桥对每条 `CoreEvent` 调 `payload_for`：`Some` → `emit` 到对应通道；`None` 直接丢弃。
3. 契约由 `tests/events_tests.rs` 锁定：两个通道名、载荷逐键、**不桥接的 11 个变体逐一列举**（与 `CoreEvent` 全部 13 个变体对账，用例内断言 `events.len() + 2 == 13`）——不给 `payload_for` 的 `_ => None` 通配留「新增变体静默落入通配、CI 不红」的口子；新增变体时必须在此表态。
