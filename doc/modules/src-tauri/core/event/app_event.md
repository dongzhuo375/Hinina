# app_event

## 职责
定义应用全局事件枚举 `AppEvent` 及其子事件类型（AuthEvent、ContestEvent、ProblemEvent、SubmissionEvent、WorkspaceEvent、SystemEvent），覆盖所有领域的状态变更事件，供 EventBus 分发。提供 `AppEvent::category()` 方法将事件映射为 `EventCategory`。

## 核心类型/函数
- **`AppEvent`** — 顶层事件枚举，变体：`Auth`, `Contest`, `Problem`, `Submission`, `Workspace`, `System`
- **`AppEvent::category()`** — 返回事件对应的 EventCategory
- **`AuthEvent`** — 认证事件：`LoginSuccess`, `Logout`, `SessionExpired`, `TokenRefreshed { token }`（Provider 侧凭证轮换，仅携带新凭证字符串，不含 OJ 私有语义；AuthService 订阅后回写磁盘会话）
- **`ContestEvent`** — 比赛事件：`ListLoaded`, `Selected`, `CountdownTick`
- **`ProblemEvent`** — 题目事件：`Opened`, `CodeChanged`
- **`SubmissionEvent`** — 提交事件：`Created`, `Judged`（评测轮询超时是前端关注点，由 submissionStore 的 createPoller 判定，原 `PollTimeout` 变体已随后端单次查询化删除）
- **`WorkspaceEvent`** — 工作区事件：`Loaded`, `Saved`, `AutoSaveTriggered`, `Switched`
- **`SystemEvent`** — 系统事件：`ConfigReloaded`, `ThemeChanged`, `OJSwitched { oj_id: String }`（payload 为 OJ 身份字符串，`commands::oj_cmd::switch_oj` 切换成功后发布）, `WindowClosing`

## 直接依赖
- `core::entity::contest::Contest`
- `core::entity::submission::JudgementResult`
- `core::entity::user::User`
- `core::event::event_category::EventCategory`

## 被依赖
- `core::event::event_bus`（EventBus 发布 AppEvent）
- `commands::oj_cmd`（`switch_oj` 成功后发布 `SystemEvent::OJSwitched`）

## 逻辑流程
纯类型定义 + `category()` 映射方法。
