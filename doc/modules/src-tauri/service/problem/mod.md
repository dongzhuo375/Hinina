# mod

## 职责
题目服务模块入口。负责题目列表获取、题目详情打开及事件发布、题面缓存（内存 + 磁盘，受配置开关控制）、用户题目状态批量查询、题目 limits 批量获取（内存 + 磁盘双层缓存）。打开题目时通过 `ProblemEvent::Opened` 事件通知调用方（通常由前端 Command 或 WorkspaceManager 响应），实现代码保留——解耦题目获取与工作区管理。

## 核心类型/函数
- `pub mod error` — 题目错误类型模块声明
- 常量：`LIMITS_CACHE_DIR = "cache/problem_limits"`（磁盘缓存目录，limits 只能从题目详情接口取得且对同一题基本不变，跨重启复用可省掉整批详情请求）、`LIMITS_CONCURRENCY = 4`（并发扇出上限：既要让首屏尽快补齐，也不能让单客户端瞬间打爆 OJ）
- 常量（公开）：`PROBLEM_CACHE_TTL = 30min`（题面缓存 TTL —— 题面在比赛期间基本不变但并非永不变化，管理员可能中途修正，30 分钟是「切题来回/重进应用几乎总能命中」与「修正最多滞后半小时」的折中）、`PROBLEM_CACHE_CAPACITY = 200`、`PROBLEM_CACHE_NAMESPACE = "cache/problem_statement"`
- 私有函数：`statement_key(contest_id, display_id) -> String` — 题面缓存键 `{contest_id}/{display_id}`（比赛维度隔离，磁盘按比赛分目录；键安全性由 `JsonDiskCache::key_path` 统一把关）
- **`ProblemService`** — 题目服务
  - `fn new(registry, event_bus, storage) -> Self` — 创建实例（`storage` 用于 limits 与题面磁盘缓存）
  - `async fn list_problems(&self, contest_id) -> AppResult<Vec<Problem>>` — 获取比赛下所有题目列表（每次从远端获取，无缓存）
  - `async fn open_problem(&self, contest_id, problem_id, cache_enabled: bool) -> AppResult<Problem>` — 获取题目详情并发布 `ProblemEvent::Opened`（含 contest_id 和 problem_id）；`cache_enabled` 来自配置 `oj.cacheProblemStatement`，**无论命中与否都照常发布事件**（WorkspaceManager 依赖它切换工作区）
  - `async fn load_problem_statement(&self, contest_id, problem_id, cache_enabled) -> AppResult<Problem>`（私有） — 题面缓存编排：内存 → 磁盘 → 网络，命中即回填上游；`cache_enabled=false` 时完全直连
  - `async fn get_user_problem_status(&self, contest_id, problem_ids: &[String]) -> AppResult<HashMap<String, i32>>` — 批量查询当前用户提交状态（key=pid，`0=未提交 / 1=已AC / 2=尝试过`，未出现的题视为未提交）；空列表直接返回空 map，不发请求
  - `async fn load_problem_limits(&self, contest_id, display_ids: &[String]) -> AppResult<Vec<ProblemLimits>>` — 批量获取题目 limits（时间 ms / 内存 MB），带内存 + 磁盘双层缓存；返回顺序与入参一致，获取失败的题在结果中**缺失**
  - 内部：`fetch_limits()`（分批并发拉详情）、`limits_cache_path()` / `read_limits_cache()` / `write_limits_cache()`
- **字段**：`registry: Arc<dyn ProviderRegistry>`, `event_bus: Arc<EventBus>`, `storage: Arc<Storage>`, `limits_cache: RwLock<HashMap<contest_id, HashMap<display_id, ProblemLimits>>>`, `statement_cache: TtlCache<String, Problem>`, `statement_disk: JsonDiskCache`

## 直接依赖
- `std::collections::HashMap`
- `std::time::Duration`（题面缓存 TTL 常量）
- `tokio::task::JoinSet`（limits 分批并发）
- `core::entity::problem::Problem`
- `core::entity::rank::ProblemLimits`
- `core::error::{AppError, AppResult}`
- `core::event::app_event::{AppEvent, ProblemEvent}`
- `core::event::event_bus::EventBus`
- `core::provider::registry::ProviderRegistry`
- `infra::cache::{JsonDiskCache, TtlCache}`（题面内存 + 磁盘缓存）
- `infra::storage::Storage`（limits / 题面磁盘缓存底层）

## 被依赖
- `core::context`（`AppContext` 持有 `Arc<ProblemService>`，装配时注入 `Arc<Storage>`）
- `commands::problem_cmd`（经 AppContext 转发 `get_problem` / `list_problems` / `get_user_problem_status` / `get_contest_problem_limits`）

## 逻辑流程
- **list_problems(contest_id)**：直接调 `ProblemProvider::list_problems()` 从远端拉取，不做本地缓存；失败 `warn!` + `e.context("获取题目列表失败")` 上抛（**变体原样穿透**）
- **open_problem(contest_id, problem_id, cache_enabled)**：`load_problem_statement`（内存 → 磁盘 → 网络，命中即回填；`cache_enabled=false` 直连）→ 发布 `ProblemEvent::Opened` → 调用方通过事件驱动 WorkspaceManager 创建或切换工作区。**只缓存成功结果**：Provider 错误原样上抛，不入缓存
- **get_user_problem_status(contest_id, problem_ids)**：空列表短路 → `registry.current_problem()` → `ProblemProvider::get_user_problem_status()` → 失败 `e.context("获取用户题目状态失败")`，变体不改写
- **load_problem_limits(contest_id, display_ids)**：

```
1. 内存缓存 limits_cache[contest_id] 整表克隆
2. 内存无该比赛任何记录 → 读磁盘 cache/problem_limits/{cid}.json
   （不存在/损坏返回空表，损坏文件会被后续回写覆盖）
3. missing = display_ids 中未命中的项
   → fetch_limits：按 LIMITS_CONCURRENCY=4 分块，每块用 JoinSet 并发调
     ProblemProvider::get_problem(cid, display_id) 取 time_limit/memory_limit
4. 错误策略：
   - 部分失败 → 逐题 warn 并跳过，该题在结果中缺失（前端显示占位而不是假默认值）
   - 全部失败 → 上抛首个错误 —— 401/403 这类会话或权限问题必须让前端明确提示，
     不能被静默吞成「拿不到 limits」（HOJ-Problem-Limits-API.md §9.5）
   - Provider 不可用 → 整批失败，交由上抛
5. 有成功项 → 合并进 resolved → 回写内存缓存 + 磁盘缓存
   （磁盘写失败只 warn：缓存是优化，不影响正确性）
6. 按 display_ids 顺序 filter_map 输出（失败题自然缺失）
```

## 设计要点
- **错误处理约定：用 `context()` 而不是重新包装**。传播 Provider 错误一律 `e.context("环节名")`（保留变体、仍补环节名、`warn!` 日志保留），**禁止** `AppError::Problem(format!("…: {}", e))` —— 变体是前端 `isAuthError` 分流与 `stores/sessionGuard.ts` 会话失效兜底的唯一依据（见 `core/error.md`）。改写成 `Problem` 后，token 过期时选手只会看到「题目错误」文案而不会被带回登录页，反复重试全部失败。
- **`load_problem_limits` 同样遵守**：401/403 原样上抛，既不回退默认值，也不改写成 `Problem` 变体。`fetch_limits` 内部只在 JoinSet 任务 join 失败时自行构造 `AppError::Unknown`（那是本层自己的错误，不存在改写下游变体的问题）。
- **`get_user_problem_status` 空入参短路**：`problem_ids` 为空时直接返回空 map，不发请求 —— 因此针对它的错误路径测试**必须传非空列表**才能真正走到 Provider。
- **题面缓存（内存 + 磁盘，TTL 30min，受开关控制）**：题面是「几乎不变但并非永不变化」的公共数据 —— 管理员可能中途修正题面/样例。故 TTL 取 30 分钟（不是永久），并给出配置开关 `oj.cacheProblemStatement`（默认开启）供「要立刻看真值」时关闭；关闭时**不读不写**（连磁盘目录都不创建）。磁盘层带 `fetchedAt`，重启后继续计时；过期文件懒删除。缓存键含 `contest_id`：同一 `display_id` 在不同比赛是不同题目。**只缓存成功结果**，401/403 不入缓存（否则会话失效被缓存掩盖）。limits 仍走自己的双层缓存，两套缓存互不干扰（`limits` 也可从题面实体派生，但保持既有实现避免重复改造）。

## 测试
`src-tauri/src/service/problem/tests/problem_tests.rs` 以桩 Provider + 临时目录锁定：limits 首次全量拉取并落盘、二次调用命中内存缓存零请求、磁盘缓存跨 Service 实例复用、部分失败只返回成功子集、**全部失败上抛错误而非回退假默认值**、损坏缓存文件重新获取、空入参短路不发请求；`get_user_problem_status` 空列表短路与按 pid 映射。

**题面缓存**（5 项）：二次 `open_problem` 命中缓存零请求、开关关闭时每次都请求且**不落盘**、磁盘缓存跨实例复用（模拟重启零请求）、按 `contest_id + display_id` 隔离（同题号不同比赛不互命中）、**错误不入缓存**（连续两次失败各请求一次，变体保持 `Auth`）。

**错误变体穿透**（3 项）：`open_problem_preserves_auth_variant`（`Auth` 变体保留且消息含「获取题目详情失败」环节名）、`list_problems_preserves_auth_variant`、`get_user_problem_status_preserves_auth_variant`（题目总览的「我的状态」每 30s 轮询一次，变体被改写会让守卫失灵；用例传非空 pid 列表以绕开空入参短路）。

为支撑这组用例，`StubProblemProvider` 新增 `fail_all: bool` 字段（为 true 时 `list_problems` / `get_user_problem_status` 也返回 `Auth` 错误），并新增 `build_service_with(dir, calls, failing, fail_all)` 构造函数；`build_service` 保持原签名（内部转调 `build_service_with(…, false)`），以免影响既有 limits 测试。`open_problem` 的失败路径复用既有 `failing` 列表机制（Stub 对列表内 displayId 返回 `Auth`，模拟 401 或私有赛未注册）。
