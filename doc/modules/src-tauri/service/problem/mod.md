# mod

## 职责
题目服务模块入口。负责题目列表获取、题目详情打开及事件发布。打开题目时通过 `ProblemEvent::Opened` 事件通知调用方（通常由前端 Command 或 WorkspaceManager 响应），实现代码保留——解耦题目获取与工作区管理。

## 核心类型/函数
- `pub mod error` — 题目错误类型模块声明
- **`ProblemService`** — 题目服务
  - `fn new(registry, event_bus) -> Self` — 创建实例
  - `async fn list_problems(&self, contest_id) -> AppResult<Vec<Problem>>` — 获取比赛下所有题目列表（每次从远端获取，无缓存）
  - `async fn open_problem(&self, contest_id, problem_id) -> AppResult<Problem>` — 获取题目详情并发布 `ProblemEvent::Opened`（含 contest_id 和 problem_id）
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<EventBus>`

## 直接依赖
- `core::entity::problem::Problem`
- `core::error::{AppError, AppResult}`
- `core::event::app_event::{AppEvent, ProblemEvent}`
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<ProblemService>`）

## 逻辑流程
- **list_problems(contest_id)**：直接调 `ProblemProvider::list_problems()` 从远端拉取，不做本地缓存
- **open_problem(contest_id, problem_id)**：调 `ProblemProvider::get_problem()` → 发布 `ProblemEvent::Opened` → 调用方通过事件驱动 WorkspaceManager 创建或切换工作区
