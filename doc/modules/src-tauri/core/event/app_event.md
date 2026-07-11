# app_event

## 职责
定义应用全局事件枚举 `AppEvent` 及其子事件类型（AuthEvent、ContestEvent、ProblemEvent、SubmissionEvent、WorkspaceEvent、SystemEvent），覆盖所有领域的状态变更事件，供 EventBus 分发。提供 `AppEvent::category()` 方法将事件映射为 `EventCategory`。

## 核心类型/函数
- **`AppEvent`** — 顶层事件枚举，变体：`Auth`, `Contest`, `Problem`, `Submission`, `Workspace`, `System`
- **`AppEvent::category()`** — 返回事件对应的 EventCategory
- **`AuthEvent`** — 认证事件：`LoginSuccess`, `Logout`, `SessionExpired`
- **`ContestEvent`** — 比赛事件：`ListLoaded`, `Selected`, `CountdownTick`
- **`ProblemEvent`** — 题目事件：`Opened`, `CodeChanged`
- **`SubmissionEvent`** — 提交事件：`Created`, `Judged`, `PollTimeout`
- **`WorkspaceEvent`** — 工作区事件：`Loaded`, `Saved`, `AutoSaveTriggered`, `Switched`
- **`SystemEvent`** — 系统事件：`ConfigReloaded`, `ThemeChanged`, `OJSwitched`, `WindowClosing`

## 直接依赖
- `core::entity::contest::Contest`
- `core::entity::submission::JudgementResult`
- `core::entity::user::User`
- `core::provider::oj_type::OJType`
- `core::event::event_category::EventCategory`

## 被依赖
- `core::event::event_bus`（EventBus 发布 AppEvent）

## 逻辑流程
纯类型定义 + `category()` 映射方法。
