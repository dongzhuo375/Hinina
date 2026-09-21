# mod

## 职责
提交服务模块入口。负责代码提交（本地留档源码快照）、评测结果单次查询，以及比赛提交列表、提交详情、测试点结果三类查询。提交从当前 Workspace 获取代码（由调用方传入）。评测轮询的节拍与总超时由前端 `submissionStore` 的 createPoller 拥有（配置 `oj.pollIntervalSecs` / `oj.pollTimeoutSecs` 经 get_config 由前端消费）；后端只做单次查询与终态事件发布，不循环、不睡眠、不设 deadline。终态提交的详情/测试点带**仅内存**的 TTL 缓存（用户域数据不落盘，登出清空；**切 OJ 亦清空** —— 由 `switch_oj` 命令显式调用 `on_oj_switched()`，换 OJ 即换用户上下文）。

## 核心类型/函数
- `pub mod error` — 提交错误类型模块声明
- `pub mod snapshot` — 提交源码快照（`submissions/{oj_id}/{submit_id}.{ext}`，见 `snapshot.md`）
- 常量：`SUBMISSION_CACHE_TTL = 2h`（终态结果不可变，长 TTL 只为最终回收；内存上界由容量保证）、`SUBMISSION_DETAIL_CAPACITY = 200`、`SUBMISSION_CASES_CAPACITY = 100`
- **`SubmissionService`** — 提交服务
  - `fn new(registry, event_bus, storage) -> Self` — 创建实例（构造三个内存缓存；`storage` 用于源码快照）
  - `fn on_oj_switched(&self)` — OJ 切换后的用户域缓存清理（**由 `switch_oj` 命令显式调用，不订阅事件**）。缓存内是**旧 OJ 用户**的提交详情与测试点（含源代码），切换后不得复用 —— 与登出清理同一语义。为什么显式：`switch_oj` 返回后新 OJ 的查询立刻可能进来，清缓存属于切换正确性的一部分，不能依赖异步投递；三个缓存都在内存里，清理本身是 µs 级，无需后台任务
  - `fn clear_user_caches(&self)` — 清空终态详情/测试点/终态标记三个缓存（`on_oj_switched` 与 `auth_cmd::logout` 共用同一实现）
  - `fn cache_key(&self, submit_id) -> String`（私有） — 缓存键 `{oj}/{submit_id}`
  - `async fn submit(&self, contest_id, problem_id, display_id, language, source_code) -> AppResult<String>` — 提交代码到 OJ，返回 submission_id。**`problem_id` 与 `display_id` 同时下传**（各 OJ 认的不是同一个标识，见 `core/provider/submission.md`）。成功后调 `snapshot::write_snapshot(...)` 本地留档源码（best-effort，失败只 warn）；发布 `CoreEvent::SubmissionCreated { submission_id }`。Provider 错误经 `e.context("提交失败")` 上抛，**变体原样穿透**
  - `async fn get_judgement(&self, submission_id) -> AppResult<JudgementResult>` — **单次**查询评测结果：每次调用只发一次 `provider.get_judgement`。终态发布 `CoreEvent::SubmissionJudged { submission_id, status }`（`status` 取 `JudgementStatus::as_str()` 的稳定字符串名 —— 载荷只放状态摘要，不放含耗时/内存/明细的整个 `JudgementResult`）；非终态（`Pending`/`Compiling`/`Running`）原样透传、不发事件，是否继续轮询由前端决定。失败 `warn!` + `e.context("评测查询失败")`，**变体原样穿透**（`Auth` 变体是前端 sessionGuard 的判据；瞬时抖动的容忍与重试由前端 poller 编排）。**不缓存**（评测中随时会变）
  - `async fn list_contest_submissions(&self, query: &SubmissionQuery) -> AppResult<SubmissionPage>` — 查询比赛提交列表（分页），经 registry 调 `SubmissionProvider::list_contest_submissions`。**不做缓存**：提交状态随时在变（评测中 → 终态），必须由前端控制刷新节奏。失败 `warn!` + `e.context("获取提交列表失败")`，变体穿透
  - `async fn get_submission_detail(&self, submit_id) -> AppResult<SubmissionDetail>` — 查询提交详情（含源代码与错误信息）。**OJ 未回吐代码（`code` 为空）时回落到本地快照**（`snapshot::read_snapshot`）。**终态结果入内存缓存**（命中零请求）；失败 `warn!` + `e.context("获取提交详情失败")`，变体穿透
  - `async fn get_submission_cases(&self, submit_id) -> AppResult<SubmissionCases>` — 查询提交的全部测试点结果。**仅当该提交已被确认终态**（`terminal_marks`）时入缓存；失败 `warn!` + `e.context("获取测试点结果失败")`，变体穿透
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<CoreEventBus>`, `storage: Arc<Storage>`, `detail_cache: Arc<TtlCache<String, SubmissionDetail>>`, `cases_cache: Arc<TtlCache<String, SubmissionCases>>`, `terminal_marks: Arc<TtlCache<String, bool>>`。三处缓存的键均为 **`{oj}/{submit_id}`**（`cache_key()` 统一构造）—— `submit_id` 是各 OJ 自增的资源号，必然重号；键带 OJ 维度后跨 OJ 串号在结构上不可能，`on_oj_switched()` 的清理因此只是让当前会话立刻干净，而不是正确性的唯一依赖

## 直接依赖
- `std::time::Duration`（缓存 TTL 常量）
- `core::entity::submission::{JudgementResult, JudgementStatus, SubmissionCases, SubmissionDetail, SubmissionPage, SubmissionQuery}`（`JudgementStatus::is_terminal` 为「是否入缓存」与「是否发判定事件」的共同判据；`as_str()` 提供事件载荷的状态字符串）
- `core::error::AppResult`（错误一律经 `e.context()` 补环节名穿透，本模块不构造错误变体）
- `core::event::core_event::CoreEvent`
- `core::event::core_event_bus::CoreEventBus`
- `core::provider::registry::ProviderRegistry`
- `infra::cache::TtlCache`（终态详情/测试点内存缓存）
- `infra::storage::Storage`（提交源码快照落盘/读取）
- `service::submission::snapshot`（快照路径构造、扩展名推导、读写）

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<SubmissionService>`，构造时传入 `storage`）
- `commands::submission_cmd`（经 AppContext 转发 `submit_code` → `submit`（含 `display_id`）、`get_judgement` → `get_judgement`、`list_contest_submissions` / `get_submission_detail` / `get_submission_cases` → 同名方法；不再读取 `OjConfig` 轮询参数）
- `commands::oj_cmd`（`switch_oj` → `on_oj_switched()`）、`commands::auth_cmd`（`logout` → `clear_user_caches()`）、`commands::maintenance_cmd`（`reset_client` → `clear_user_caches()`）

## 逻辑流程
- **submit**：调 `SubmissionProvider::submit(contest_id, problem_id, display_id, language, source_code)`（失败 `warn!` + `e.context("提交失败")`，变体不改写）→ `snapshot::write_snapshot(storage, oj_id, submission_id, language, source_code)`（best-effort）→ 发布 `CoreEvent::SubmissionCreated { submission_id }` → 返回 submission_id
- **get_judgement**：`registry.current_submission()` → 单次调 `provider.get_judgement(id)`（失败 `warn!` + `e.context("评测查询失败")` 上抛，变体穿透）→ 按状态分流：
  - 终态（非 `Pending`/`Compiling`/`Running`）→ 发布 `CoreEvent::SubmissionJudged { submission_id, status: result.status.as_str() }` → 返回结果
  - 非终态 → 不发事件，原样透传结果（Provider 的 `get_judgement` 原样透传非终态，排队中为 `Pending`），由前端 poller 决定是否继续下一拍
- **on_oj_switched / clear_user_caches**：清空 `detail_cache` / `cases_cache` / `terminal_marks` 三个内存缓存
- **list_contest_submissions / get_submission_detail / get_submission_cases**：`registry.current_submission()` → 调 Provider 同名方法 → 失败 `warn!` + `e.context(环节名)` 上抛（**变体原样穿透**）→ 成功透传实体。列表查询不缓存（提交状态随时在变，刷新节奏由前端控制）；详情与测试点按终态缓存（见下）
- **get_submission_detail**：命中 `detail_cache` → 直接返回（零请求）；未命中 → 调 Provider → **`code` 为空时回落到本地快照**（`snapshot::read_snapshot`，先精确路径后按 `{submit_id}.*` 扫描）→ **仅当 `status.is_terminal()`** 时写入 `detail_cache` 并留下 `terminal_marks` 标记
- **get_submission_cases**：命中 `cases_cache` → 直接返回；未命中 → 调 Provider → **仅当 `terminal_marks` 存在该 submit_id** 时写入 `cases_cache`

## 设计要点
- **终态详情/测试点缓存（仅内存）**：终态结果不可变，而「列表 → 详情 → 返回 → 再进」是现场高频动作，每次重拉纯属浪费；TTL 2h 只为最终回收，真正的内存上界由容量上限（200 / 100）保证。**评测中的结果永不缓存**：详情会变（状态/耗时/内存/错误信息），测试点是半截明细 —— 缓存它会让「评测完成后打开详情页」看到缺失的测试点。
- **测试点的终态判据用 `terminal_marks` 而非再查一次详情**：测试点接口不返回状态，无法自证可缓存。详情页流程天然「先拉详情（终态即留标记）再拉测试点」，故此处可命中；而控制台条的失败测试点提示**直接拉测试点、无详情上下文**，此时不缓存 —— 保持与改造前一致的请求数，而不是为判定终态多发一次详情请求。`clear_user_caches()` 同时清标记，登出后需重新建立上下文。
- **用户域数据不落盘 + 登出即清 + 切 OJ 即清**：缓存内容含**源代码**，落盘会让同机换账号后仍能读到上一位选手的提交；故只用 `TtlCache`（内存），并由命令层显式编排清理 —— `auth_cmd::logout` 调 `clear_user_caches()`，`oj_cmd::switch_oj` 调 `on_oj_switched()`（换 OJ 即换用户上下文，旧 OJ 用户的源码不得复用）。两者共用同一实现，且都**不依赖事件投递**：`switch_oj` 返回后新 OJ 的查询立刻可能进来，清缓存属于切换正确性的一部分。**唯一的例外是提交源码快照**：它按 `submissions/{oj_id}/{submit_id}.{ext}` 落盘，因为「我当时交的是什么」属于本地事实 —— OJ 会在比赛隐藏记录 / `codeShare=false` / 赛后回收等情形下不回吐代码，提交失败时更彻底（服务端一行记录都没有）。带 OJ 维度即避免跨 OJ 撞号，落盘失败只告警不阻断提交
- **提交参数同时收 `problem_id` 与 `display_id`**：HOJ 的 `POST /submit-problem-judge` 收的是**比赛内展示题号**（服务端拿它查 `contest_problem.display_id`，查不到直接 NPE 返回 HTTP 500 —— 实测传数字 pid 必 500），Hydro 的 `/p/{id}/submit` 收的则是真实 ID。Service 不做任何换算，两个都往下传，由 Adapter 各取所需。
- **错误处理约定：用 `context()` 而不是重新包装**。传播 Provider 错误一律 `e.context("环节名")`（保留变体、仍补环节名、`warn!` 日志保留），**禁止** `AppError::Submission(format!("…: {}", e))` —— 变体是前端 `isAuthError` 分流与 `guards/sessionGuard.ts` 会话失效兜底的唯一依据（见 `core/error.md`）。提交是赛场上最不能失败的操作：token 过期时若被改写成 `Submission` 变体，选手只会看到一条「提交失败」文案，反复重试全部失败，却永远不会被带回登录页。同理，断网（`Network`）也不能显示成「提交被拒」。
- **后端单次查询、前端拥有节拍**（评审方案①）。旧版 `poll_judgement` 是 Rust 侧阻塞循环（固定 2s 间隔、无抖动、最长 300s），而前端 `submissionStore` 本就用 createPoller（2s ±20% 抖动 + 重入守卫 + 前端 deadline）包裹该命令 —— 每个前端 tick 都阻塞到终态，前端抖动形同虚设，真实请求节奏是后端的固定 2s，且前端 `stopPolling`（登出）无法终止在飞的后端循环。改为单次查询后：前端 poller 成为唯一节拍所有者（终态停止、总超时、瞬时错误容忍均已在 poller 内实现），并与同为单次查询的 `get_submission_detail` 语义统一。旧版的超时事件随之删除（超时是前端关注点，事件无订阅者）。

## 测试
`src-tauri/src/service/submission/tests/submission_tests.rs`（由 `mod.rs` 底部 `#[cfg(test)] #[path = "tests/submission_tests.rs"] mod tests;` 引用）以 `StubSubmissionProvider` 锁定变体穿透与单次查询契约。Stub 用 `StubMode::{Ok, Auth, Network, AlwaysRunning, NonTerminalDetail}` 覆盖成功（终态 CompilationError）、token 过期、断网、非终态评测、非终态详情五种情形，并用 `Arc<AtomicUsize>` 分别记录 `get_judgement` / `get_submission_detail` / `get_submission_cases` 的调用次数（`build_service` 返回 `StubCounters` 句柄；`make_service` 保持旧签名以免影响既有用例）；`build_service` 额外返回 `Arc<CoreEventBus>`，配合 `collect_submission_events` / `drain_submission_events` 断言事件契约；异步用例各自建 current-thread runtime 避免嵌套 panic。

**终态缓存用例**（8 项）：终态详情命中缓存、非终态详情每次重拉、错误不入缓存、有终态标记时测试点命中缓存、无标记（控制台条路径）不缓存、非终态详情不使测试点可缓存、按 submit_id 隔离、`clear_user_caches` 后详情与测试点均需重拉。

> 本模块另依赖 `tests/snapshot_tests.rs` 覆盖快照（扩展名与前端对齐、路径越权拒绝、往返读取、语言变更后扫描回退、跨 OJ 不撞号、落盘失败不 panic）。测试用 `temp_storage()` 为每次 `build_service` 生成独立存储根，避免并行用例相互覆盖快照。

覆盖：`submit` 保留 `Auth` 变体（且消息含「提交失败」环节名）与 `Network` 变体；`get_judgement` 成功路径**原样透传** `JudgementResult`（score/time/memory 完整、只调 Provider 一次）；`Auth` / `Network` 变体穿透（`Auth` 消息含「评测查询失败」环节名、后端不重试只查一次）；**终态发布且仅发布一条 `SubmissionJudged`（携带 submission_id 与状态字符串）**；**非终态（Running）原样透传且不发任何事件**。

**事件契约**：`submit_publishes_submission_created`（提交成功后发布 `SubmissionCreated`，载荷只含 submission_id）；`on_oj_switched_clears_user_scoped_caches`（切 OJ 后三个用户域缓存全空、必须回源）。

**提交列表 / 详情 / 测试点**（同一 Stub 实现全部五个 trait 方法）：`list_contest_submissions` 保留 `Auth` 变体且成功路径返回分页、`get_submission_detail` 保留 `Auth` 且成功路径返回完整实体、`get_submission_cases` 保留 `Auth` / `Network` 变体且成功路径返回测试点列表 —— 锁定三个查询方法与 `submit`/`get_judgement` 遵守同一「`e.context()` 补环节名、变体不改写」约定。
