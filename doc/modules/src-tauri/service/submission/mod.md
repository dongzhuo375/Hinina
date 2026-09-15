# mod

## 职责
提交服务模块入口。负责代码提交、评测结果轮询及超时处理。提交从当前 Workspace 获取代码（由调用方传入），轮询间隔和超时由 Config 控制（通过 `OjConfig::poll_interval_secs` / `poll_timeout_secs`）。

## 核心类型/函数
- `pub mod error` — 提交错误类型模块声明
- **`SubmissionService`** — 提交服务
  - `fn new(registry, event_bus) -> Self` — 创建实例
  - `async fn submit(&self, contest_id, problem_id, language, source_code) -> AppResult<String>` — 提交代码到 OJ，返回 submission_id；发布 `SubmissionEvent::Created`。Provider 错误经 `e.context("提交失败")` 上抛，**变体原样穿透**
  - `async fn poll_judgement(&self, submission_id, poll_interval_secs, poll_timeout_secs) -> AppResult<JudgementResult>` — 轮询评测结果：按 interval 循环查询，timeout 前获取到终态则发布 `SubmissionEvent::Judged`；超时则发布 `SubmissionEvent::PollTimeout` 并返回 `AppError::Submission("评测超时: …")`。**认证错误立即短路**：`get_judgement` 返回 `AppError::Auth(_)` 时不重试，直接 `Err(e.context("评测查询失败"))`；其它错误（瞬时抖动）保持原有「仅告警并重试到超时」语义
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<EventBus>`

## 直接依赖
- `core::entity::submission::{JudgementResult, JudgementStatus}`（`Running` 为继续轮询的判据）
- `core::error::{AppError, AppResult}`（`AppError::Submission` 用于轮询超时；`AppError::Auth` 用于 `matches!` 短路判定）
- `core::event::app_event::{AppEvent, SubmissionEvent}`
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`
- `tokio::time::{sleep, Instant}`（轮询节奏与 deadline）

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<SubmissionService>`）
- `commands::submission_cmd`（经 AppContext 转发 `submit_code` → `submit`、`get_judgement` → `poll_judgement`；轮询参数取自 `OjConfig::poll_interval_secs` / `poll_timeout_secs`，默认 **2s / 300s**）

## 逻辑流程
- **submit**：调 `SubmissionProvider::submit()`（失败 `warn!` + `e.context("提交失败")`，变体不改写）→ 发布 `SubmissionEvent::Created { submission_id }` → 返回 submission_id
- **poll_judgement**：计算 deadline = now + timeout → 循环：先判超时（超时则发布 `PollTimeout { submission_id }` 并返回 `AppError::Submission`）→ `provider.get_judgement(id)`：
  - `Ok` 且为终态 → 发布 `Judged { submission_id, result }` → 返回
  - `Ok` 且为 `Running` → 继续轮询
  - `Err(AppError::Auth(_))` → **立即上抛** `e.context("评测查询失败")`，不再轮询
  - `Err(其它)` → 仅 `warn!` 并重试（瞬时抖动）
  - 每轮末尾 `sleep(poll_interval_secs)`

## 设计要点
- **错误处理约定：用 `context()` 而不是重新包装**。传播 Provider 错误一律 `e.context("环节名")`（保留变体、仍补环节名、`warn!` 日志保留），**禁止** `AppError::Submission(format!("…: {}", e))` —— 变体是前端 `isAuthError` 分流与 `stores/sessionGuard.ts` 会话失效兜底的唯一依据（见 `core/error.md`）。提交是赛场上最不能失败的操作：token 过期时若被改写成 `Submission` 变体，选手只会看到一条「提交失败」文案，反复重试全部失败，却永远不会被带回登录页。同理，断网（`Network`）也不能显示成「提交被拒」。
- **`poll_judgement` 不得吞掉认证错误**。`commands/submission_cmd.rs` 的 `get_judgement` command 传入的 `poll_timeout_secs` 默认 **300s**，若把 `Auth` 当瞬时错误重试，token 过期时选手会**干等五分钟**，最后收到「评测超时」（`Submission` 变体）—— 既慢，又把会话失效伪装成评测问题，前端守卫拿不到 `Auth` 变体。认证错误重试也只是反复 401，故立即短路。
- **超时归 `Submission` 变体**：一直 `Running` 到 deadline 是评测语义问题（不是会话问题），必须是 `Submission` 才能与 `Auth` 区分开。

## 测试
`src-tauri/src/service/submission/tests/submission_tests.rs`（由 `mod.rs` 底部 `#[cfg(test)] #[path = "tests/submission_tests.rs"] mod tests;` 引用）以 `StubSubmissionProvider` 锁定变体穿透与轮询语义。Stub 用 `StubMode::{Ok, Auth, Network, AlwaysRunning, FlakyThenOk(n)}` 覆盖成功、token 过期、断网、永远 `Running`、前 n 次抖动后恢复五种情形，并用 `Arc<AtomicUsize>` 记录 `get_judgement` 调用次数；异步用例各自建 current-thread runtime 避免嵌套 panic。

覆盖：`submit` 保留 `Auth` 变体（且消息含「提交失败」环节名）与 `Network` 变体；`poll_judgement` 遇认证错误**立即上抛 `Auth`、只调用 Provider 一次、5s 内返回**（用例故意把 timeout 给到 300s、interval 给到 60s —— 若认证错误被当瞬时错误重试，这里会挂住五分钟并报「评测超时」）；瞬时抖动 `FlakyThenOk(2)` 仍重试后成功（共 3 次调用，拿到 `Accepted` 与 time/memory）；一直 `Running` 时超时错误为 `Submission` 变体且消息含「评测超时」；终态一次返回不再轮询（调用次数为 1）。
