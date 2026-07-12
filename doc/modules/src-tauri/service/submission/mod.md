# mod

## 职责
提交服务模块入口。负责代码提交、评测结果轮询及超时处理。提交从当前 Workspace 获取代码（由调用方传入），轮询间隔和超时由 Config 控制（通过 `OjConfig::poll_interval_secs` / `poll_timeout_secs`）。

## 核心类型/函数
- `pub mod error` — 提交错误类型模块声明
- **`SubmissionService`** — 提交服务
  - `fn new(registry, event_bus) -> Self` — 创建实例
  - `async fn submit(&self, contest_id, problem_id, language, source_code) -> AppResult<String>` — 提交代码到 OJ，返回 submission_id；发布 `SubmissionEvent::Created`
  - `async fn poll_judgement(&self, submission_id, poll_interval_secs, poll_timeout_secs) -> AppResult<JudgementResult>` — 轮询评测结果：按 interval 循环查询，timeout 前获取到结果则发布 `SubmissionEvent::Judged`；超时则发布 `SubmissionEvent::PollTimeout` 并返回错误
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<EventBus>`

## 直接依赖
- `core::entity::submission::JudgementResult`
- `core::error::{AppError, AppResult}`
- `core::event::app_event::{AppEvent, SubmissionEvent}`
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<SubmissionService>`）

## 逻辑流程
- **submit**：调 `SubmissionProvider::submit()` → 发布 `SubmissionEvent::Created { submission_id }` → 返回 submission_id
- **poll_judgement**：计算 deadline = now + timeout → 循环：`tokio::time::sleep(interval)` → `provider.get_judgement(id)` → 成功则发布 `Judged { submission_id, result }` → 返回；超时则发布 `PollTimeout { submission_id }` → 返回错误；单个查询失败仅警告并重试
