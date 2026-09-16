# mod

## 职责
提交服务模块入口。负责代码提交、评测结果单次查询，以及比赛提交列表、提交详情、测试点结果三类查询。提交从当前 Workspace 获取代码（由调用方传入）。评测轮询的节拍与总超时由前端 `submissionStore` 的 createPoller 拥有（配置 `oj.pollIntervalSecs` / `oj.pollTimeoutSecs` 经 get_config 由前端消费）；后端只做单次查询与终态事件发布，不循环、不睡眠、不设 deadline。

## 核心类型/函数
- `pub mod error` — 提交错误类型模块声明
- **`SubmissionService`** — 提交服务
  - `fn new(registry, event_bus) -> Self` — 创建实例
  - `async fn submit(&self, contest_id, problem_id, language, source_code) -> AppResult<String>` — 提交代码到 OJ，返回 submission_id；发布 `SubmissionEvent::Created`。Provider 错误经 `e.context("提交失败")` 上抛，**变体原样穿透**
  - `async fn get_judgement(&self, submission_id) -> AppResult<JudgementResult>` — **单次**查询评测结果：每次调用只发一次 `provider.get_judgement`。终态发布 `SubmissionEvent::Judged`；非终态（`Pending`/`Compiling`/`Running`）原样透传、不发事件，是否继续轮询由前端决定。失败 `warn!` + `e.context("评测查询失败")`，**变体原样穿透**（`Auth` 变体是前端 sessionGuard 的判据；瞬时抖动的容忍与重试由前端 poller 编排）
  - `async fn list_contest_submissions(&self, query: &SubmissionQuery) -> AppResult<SubmissionPage>` — 查询比赛提交列表（分页），经 registry 调 `SubmissionProvider::list_contest_submissions`。**不做缓存**：提交状态随时在变（评测中 → 终态），必须由前端控制刷新节奏。失败 `warn!` + `e.context("获取提交列表失败")`，变体穿透
  - `async fn get_submission_detail(&self, submit_id) -> AppResult<SubmissionDetail>` — 查询提交详情（含源代码与错误信息）。失败 `warn!` + `e.context("获取提交详情失败")`，变体穿透
  - `async fn get_submission_cases(&self, submit_id) -> AppResult<SubmissionCases>` — 查询提交的全部测试点结果。失败 `warn!` + `e.context("获取测试点结果失败")`，变体穿透
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<EventBus>`

## 直接依赖
- `core::entity::submission::{JudgementResult, JudgementStatus, SubmissionCases, SubmissionDetail, SubmissionPage, SubmissionQuery}`（非终态 `Pending`/`Compiling`/`Running` 为「不发 Judged 事件」的判据）
- `core::error::AppResult`（错误一律经 `e.context()` 补环节名穿透，本模块不构造错误变体）
- `core::event::app_event::{AppEvent, SubmissionEvent}`
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<SubmissionService>`）
- `commands::submission_cmd`（经 AppContext 转发 `submit_code` → `submit`、`get_judgement` → `get_judgement`、`list_contest_submissions` / `get_submission_detail` / `get_submission_cases` → 同名方法；不再读取 `OjConfig` 轮询参数）

## 逻辑流程
- **submit**：调 `SubmissionProvider::submit()`（失败 `warn!` + `e.context("提交失败")`，变体不改写）→ 发布 `SubmissionEvent::Created { submission_id }` → 返回 submission_id
- **get_judgement**：`registry.get_submission()` → 单次调 `provider.get_judgement(id)`（失败 `warn!` + `e.context("评测查询失败")` 上抛，变体穿透）→ 按状态分流：
  - 终态（非 `Pending`/`Compiling`/`Running`）→ 发布 `Judged { submission_id, result }` → 返回结果
  - 非终态 → 不发事件，原样透传结果（Provider 的 `get_judgement` 原样透传非终态，排队中为 `Pending`），由前端 poller 决定是否继续下一拍
- **list_contest_submissions / get_submission_detail / get_submission_cases**：`registry.get_submission()` → 调 Provider 同名方法 → 失败 `warn!` + `e.context(环节名)` 上抛（**变体原样穿透**）→ 成功透传实体。三个查询方法均为无状态直通（不缓存、不发事件）：提交状态随时在变，刷新节奏由前端控制

## 设计要点
- **错误处理约定：用 `context()` 而不是重新包装**。传播 Provider 错误一律 `e.context("环节名")`（保留变体、仍补环节名、`warn!` 日志保留），**禁止** `AppError::Submission(format!("…: {}", e))` —— 变体是前端 `isAuthError` 分流与 `stores/sessionGuard.ts` 会话失效兜底的唯一依据（见 `core/error.md`）。提交是赛场上最不能失败的操作：token 过期时若被改写成 `Submission` 变体，选手只会看到一条「提交失败」文案，反复重试全部失败，却永远不会被带回登录页。同理，断网（`Network`）也不能显示成「提交被拒」。
- **后端单次查询、前端拥有节拍**（评审方案①）。旧版 `poll_judgement` 是 Rust 侧阻塞循环（固定 2s 间隔、无抖动、最长 300s），而前端 `submissionStore` 本就用 createPoller（2s ±20% 抖动 + 重入守卫 + 前端 deadline）包裹该命令 —— 每个前端 tick 都阻塞到终态，前端抖动形同虚设，真实请求节奏是后端的固定 2s，且前端 `stopPolling`（登出）无法终止在飞的后端循环。改为单次查询后：前端 poller 成为唯一节拍所有者（终态停止、总超时、瞬时错误容忍均已在 poller 内实现），并与同为单次查询的 `get_submission_detail` 语义统一。`SubmissionEvent::PollTimeout` 随之删除（超时是前端关注点，该事件无订阅者）。

## 测试
`src-tauri/src/service/submission/tests/submission_tests.rs`（由 `mod.rs` 底部 `#[cfg(test)] #[path = "tests/submission_tests.rs"] mod tests;` 引用）以 `StubSubmissionProvider` 锁定变体穿透与单次查询契约。Stub 用 `StubMode::{Ok, Auth, Network, AlwaysRunning}` 覆盖成功（终态 Accepted）、token 过期、断网、非终态（Running）四种情形，并用 `Arc<AtomicUsize>` 记录 `get_judgement` 调用次数；`make_service` 额外返回 `Arc<EventBus>`，配合 `collect_submission_events` 订阅器断言事件契约；异步用例各自建 current-thread runtime 避免嵌套 panic。

覆盖：`submit` 保留 `Auth` 变体（且消息含「提交失败」环节名）与 `Network` 变体；`get_judgement` 成功路径**原样透传** `JudgementResult`（score/time/memory 完整、只调 Provider 一次）；`Auth` / `Network` 变体穿透（`Auth` 消息含「评测查询失败」环节名、后端不重试只查一次）；**终态发布且仅发布一条 `Judged`（携带 submission_id）**；**非终态（Running）原样透传且不发任何事件**。

**提交列表 / 详情 / 测试点**（同一 Stub 实现全部五个 trait 方法）：`list_contest_submissions` 保留 `Auth` 变体且成功路径返回分页、`get_submission_detail` 保留 `Auth` 且成功路径返回完整实体、`get_submission_cases` 保留 `Auth` / `Network` 变体且成功路径返回测试点列表 —— 锁定三个查询方法与 `submit`/`get_judgement` 遵守同一「`e.context()` 补环节名、变体不改写」约定。
